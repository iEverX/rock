extern crate chrono;

use std::sync::Arc;
use std::net::*;
use std::thread;
use std::io::*;
use std::str;
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs::File;
use self::chrono::Local;

use config;

macro_rules! get {
    ( $expr : expr ) => {
        match $expr {
            Some(v) => v,
            None => return None,
        }
    }
}

struct Request {
    method: String,
    path: String,
    version: String,
    headers: HashMap<String, String>,
    query: Option<HashMap<String, String>>,
}

impl Request {
    fn parse(stream: &mut TcpStream) -> Option<Request> {
        let mut s = Vec::new();
        Self::get_request(stream, &mut s);
        match String::from_utf8(s) {
            Ok(s) => {
                let mut lines = s.split("\r\n");
                let values: Vec<_> = get!(lines.next()).split(' ').collect();
                if values.len() == 3 {
                    let (path, query) = Self::parse_resource(values[1]);
                    let headers: HashMap<_, _> = lines.flat_map(Self::parse_header).collect();
                    Some(Request {
                        method: values[0].to_string(),
                        path: path,
                        version: values[2].to_string(),
                        headers: headers,
                        query: query,
                    })
                } else {
                    None
                }
            },
            Err(_) => None,
        }
    }

    fn log(&self) {
        println!("{} - {} {}", Local::now().format("%Y-%m-%d %H:%M:%S"),
                 self.method, self.path);
        println!("{:?}", self.query);
    }

    fn parse_resource(resource: &str) -> (String, Option<HashMap<String, String>>) {
        let parts: Vec<_> = resource.splitn(2, '?').collect();
        if parts.len() == 1 || parts[1].trim().chars().count() == 0 {
            (parts[0].to_string(), None)
        } else {
            (parts[0].to_string(), Self::parse_query(parts[1]))
        }
    }

    fn parse_query(q: &str) -> Option<HashMap<String, String>> {
        let mut query: HashMap<String, String> = HashMap::new();
        let mut it = q.split('&');
        while let Some(kv) = it.next() {
            let mut it = kv.split('=');
            if let Some(k) = it.next() {
                if let Some(v) = it.next() {
                    query.insert(k.to_string(), v.to_string());
                }
            }
        }
        if query.is_empty() {
            None
        } else {
            Some(query)
        }
    }

    fn parse_header(line: &str) -> Option<(String, String)> {
        let mut it = line.splitn(2, ": ");
        let header = get!(it.next());
        let value = get!(it.next());
        Some((header.to_string(), value.to_string()))
    }

    fn get_request(stream: &mut TcpStream, r: &mut Vec<u8>) {
        const CHUNK_SIZE: usize = 4096;
        let mut buf = [0; CHUNK_SIZE];
        while let Ok(n) = stream.read(&mut buf) {
            r.extend_from_slice(&buf[0..n]);
            if n != CHUNK_SIZE {
                return;
            }
        }
    }
}

struct Response {
    head: String,
    body: String
}

impl Response {
    fn new(code: u16, mime: &str, content: String) -> Response {
        Self::with_head_body(Self::header(code, mime, content.chars().count()), content)
    }

    fn with_head_body(head: String, body: String) -> Response {
        Response {
            head: head,
            body: body
        }
    }

    fn code404() -> Response {
        let body = "<html><head><title>404 Not Found</title></head><body>404 Not Found</body></html>";
        Self::with_head_body(Self::header(404, "text/html", body.chars().count()), body.to_string())
    }

    fn code501() -> Response {
        let body = "<html><head><title>501 Not Implemented</title></head><body>501 Not Implemented</body></html>";
        Self::with_head_body(Self::header(501, "text/html", body.chars().count()), body.to_string())
    }

    fn send(self, mut stream: TcpStream) {
        match write!(stream, "{}\r\n{}", self.head, self.body) {
            Err(e) => println!("Response error: {}", e),
            _ => {},
        }
    }

    fn send_head(self, mut stream: TcpStream) {
        match write!(stream, "{}", self.head) {
            Err(e) => println!("Response error: {}", e),
            _ => {},
        }
    }

    fn header(code: u16, mime: &str, length: usize) -> String {
        let m = match code {
            200 => "OK",
            404 => "Not Found",
            _ => "Not Implemented"
        };
        format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n",
                code, m, mime, length)
    }
}

pub struct Rock {
    host: String,
    port: u16,
    config: config::RockConfig,
}

impl Rock {
    pub fn new(c: config::RockConfig) -> Rock {
        Rock {
            host: c.host.to_string(),
            port: c.port,
            config: c,
        }
    }

    pub fn start(self) {
        println!("Start listening at {}:{}", &self.host[..], self.port);
        let rock: Arc<Rock> = Arc::new(self);
        match TcpListener::bind((&rock.host[..], rock.port)) {
            Ok(listener) => {
                for stream in listener.incoming() {
                    match stream {
                        Err(e) => {
                            println!("Accept error {}", e);
                        },
                        Ok(s) => {
                            let shared = rock.clone();
                            thread::spawn(move || shared.handle_client(s));
                        },
                    }
                }
                drop(listener);
            },
            Err(e) => {
                println!("start server at {}:{} failed. {}", rock.host, rock.port, e);
            }
        }
    }

    fn handle_client(&self, mut stream: TcpStream) {
        if let Some(req) = Request::parse(&mut stream) {
            req.log();
            self.serve(stream, req);
        }
    }

    fn serve(&self, stream: TcpStream, req: Request) {
        match req.method.as_str() {
            "GET" => self.static_response(&req.path).send(stream),
            "HEAD" => self.static_response(&req.path).send_head(stream),
            _ => Response::code501().send(stream),
        }
    }

    fn static_response(&self, path: &String) -> Response {
        let mut buf = PathBuf::from(&self.config.root);
        let p = match path.chars().count() {
            1 => "index.html".to_string(),
            _ => path.chars().skip(1).collect(),
        };
        buf.push(p);
        match buf.as_path().to_str() {
            Some(path) => {
                match File::open(path) {
                    Ok(mut file) => {
                        let mut body = String::new();
                        file.read_to_string(&mut body).unwrap();
                        Response::new(200, "text/html", body)
                    },
                    Err(_) => {
                        Response::code404()
                    }
                }
            },
            None => {
                Response::code404()
            }
        }
    }
}