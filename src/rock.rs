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
}

impl Request {
    fn parse(stream: &mut TcpStream) -> Option<Request> {
        let mut s = Vec::new();
        Request::get_request(stream, &mut s);        
        match String::from_utf8(s) {
            Ok(s) => {
                let mut lines = s.split("\r\n");
                let values: Vec<_> = get!(lines.next()).split(' ').collect();
                if values.len() == 3 {
                    let headers: HashMap<_,_> = lines.flat_map(Request::parse_header).collect();
                    Some(Request {
                        method: values[0].to_string(),
                        path: values[1].to_string(),
                        version: values[2].to_string(),
                        headers: headers,
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
            self.method,  self.path);
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

struct Response;

impl Response {
    fn header(code: u16, mime: &str, length: usize) -> String {
        let m = match code {
            200 => "OK",
            404 => "Not Found",
            _ => "Not Implemented"
        };
        format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}",
            code, m, mime, length)
    }

    fn make(code: u16, mime: &str, content: &String) -> String {
        let header = Self::header(code, mime, content.chars().count());
        format!("{}\r\n\r\n{}", header, *content)
    }

    fn send(mut stream: TcpStream, code: u16, mime: &str, content: &String) {
        match write!(stream, "{}", Self::make(code, mime, content)) {
            Err(e) => println!("Response error: {}", e),
            _ => {},
        }
    }

    fn send_not_found(stream: TcpStream) {
        let body = "<html><head><title>404 Not Found</title></head><body>404 Not Found</body></html>";
        Self::send(stream, 404, "text/html", &body.to_string());
    }

    fn send_not_implemented(stream: TcpStream) {
        let body = "<html><head><title>501 Not Implemented</title></head><body>501 Not Implemented</body></html>";
        Self::send(stream, 404, "text/html", &body.to_string());
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
                            println!("Accept erro {}", e);
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
            match req.method.as_str() {
                "GET" => self.serve_static(stream, &req.path),
                "HEAD" => self.serve_static(stream, &req.path),
                _ => Response::send_not_implemented(stream),
            }
        }
    }

    fn serve_static(&self, stream: TcpStream, path: &String) {
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
                        Response::send(stream, 200, "text/html", &body);
                    },
                    Err(_) => {
                        Response::send_not_found(stream);
                    }
                }
            }, 
            None => {
                Response::send_not_found(stream);
            }
        }
    }
}