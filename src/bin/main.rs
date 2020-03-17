/* See LICENSE for license details */
use server::Server;

fn main() -> std::io::Result<()> {
    let server = Server::new(5, "configs.txt");
    server.start_at("0.0.0.0:8080").join().unwrap();
    Ok(())
}
