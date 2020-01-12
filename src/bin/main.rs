/* See LICENSE for license details */
use server::Server;

fn main() -> std::io::Result<()> {
    let server = Server::new(5);
    let thread = server.start_at("127.0.0.1:8080".to_string());
    thread.join().unwrap();
    Ok(())
}
