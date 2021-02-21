use std::env;
pub struct ServerConfig {
    pub port: u16,
}

impl ServerConfig {
    pub fn new(mut args: env::Args) -> Result<ServerConfig, &'static str> {
        args.next();

        let port = match args.next() {
            Some(port) => port,
            None => return Err("No port specified"),
        };

        let port = port
            .parse()
            .expect("Port should be a 16 bit unsigned integer");

        Ok(ServerConfig { port })
    }
}
