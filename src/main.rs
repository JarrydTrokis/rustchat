use std::net::TcpListener;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:1111").unwrap();
8080
    for stream in listener.incoming() {
        let stream = stream.unwrap();

        print!("we have connection");
    }
}

