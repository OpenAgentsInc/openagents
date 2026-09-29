mod routes;

fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    println!("orders-api listening on {port}");
    routes::serve(&port);
}
