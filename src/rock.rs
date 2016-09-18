extern crate chrono;
use config;

use std::sync::Arc;
use std::net::*;
use std::thread;
use std::io::*;
use std::str;
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs::File;
use self::chrono::Local;

struct Request {
    method: String,
    path: String,
    version: String,
    headers: HashMap<String, String>,
}

impl Request {
    fn parse(stream: &mut TcpStream) -> Option<Request> {
        let mut s = Vec::new();
        get_request(stream, &mut s);
        if let Ok(s) = String::from_utf8(s) {
            let mut lines = s.split("\r\n");
            if let Some(request_line) = lines.next() {
                let mut it = request_line.split(' ');
                let values: Vec<_> = it.collect();
                let headers: HashMap<_,_> = lines.flat_map(parse_header).collect();
                return Some(Request {
                    method: values[0].to_string(),
                    path: values[1].to_string(),
                    version: values[2].to_string(),
                    headers: headers,
                });
            }
        }
        None
    }

    fn log(&self) {
        println!("{} - {} {}{}", Local::now().format("%Y-%m-%d %H:%M:%S"), 
            self.method, self.headers.get("Host").unwrap(), self.path);
    }
}

fn get_request(stream: &mut TcpStream, r: &mut Vec<u8>) {
    const CHUNK_SIZE: usize = 4096;
    let mut buf = [0; CHUNK_SIZE];
    while let Ok(n) = stream.read(&mut buf) {
        r.extend_from_slice(&buf[0..n]);
        if n != CHUNK_SIZE {
            break;
        }
    }
}

fn parse_header(line: &str) -> Option<(String, String)> {
    let mut it = line.splitn(2, ": ");
    if let Some(header) = it.next() {
        if let Some(value) = it.next() {
            return Some((header.to_string(), value.to_string()));
        }
    }
    None
}

pub struct Rock {
    host: String,
    port: u16,
    config: config::RockConfig,
}

fn handle_client(rock: Arc<Rock>, mut stream: TcpStream) {
    if let Some(req) = Request::parse(&mut stream) {
        req.log();
        serve_static(rock, stream, &req.path);
        // let body = "<html><head><title>Error</title></head><body>body failed</body></html>";
        // write!(stream, "HTTP/1.0 {} {}\r\n", 200, "OK");
        // write!(stream, "Content-type: text/html\r\n");
        // write!(stream, "Content-length: {}\r\n\r\n", body.chars().count());    
        // write!(stream, "{}", body);
    }
}


fn serve_static(rock: Arc<Rock>, mut stream: TcpStream, path: &String) {
    let mut buf = PathBuf::from(&rock.config.root);
    buf.push(path.chars().skip(1).collect::<String>());
    match buf.as_path().to_str() {
        Some(path) => {
            match File::open(path) {
                Ok(mut file) => {
                    let mut body = String::new();
                    let size = file.read_to_string(&mut body).unwrap();
                    write!(stream, "HTTP/1.1 {} {}\r\n", 200, "OK");
                    write!(stream, "Content-type: text/html\r\n");
                    write!(stream, "Content-length: {}\r\n\r\n", size);    
                    write!(stream, "{}", body);
                },
                Err(e) => {
                    let body = format!("<html><head><title>Error</title></head><body>{}</body></html>", "404 Not Found");
                    write!(stream, "HTTP/1.1 {} {}\r\n", 200, "OK");
                    write!(stream, "Content-type: text/html\r\n");
                    write!(stream, "Content-length: {}\r\n\r\n", body.chars().count());    
                    write!(stream, "{}", body);
                }
            }
        }, 
        None => {
            let body = format!("<html><head><title>Error</title></head><body>{}</body></html>", "404 Not Found");
            write!(stream, "HTTP/1.1 {} {}\r\n", 200, "OK");
            write!(stream, "Content-type: text/html\r\n");
            write!(stream, "Content-length: {}\r\n\r\n", body.chars().count());    
            write!(stream, "{}", body);
        }
    }

    
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
                            thread::spawn(move || handle_client(shared, s));
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
}