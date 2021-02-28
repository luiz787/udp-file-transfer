use std::env;
pub struct ServerConfig {
    pub port: u16,
}

impl ServerConfig {
    pub fn new(mut args: env::Args) -> Result<ServerConfig, &'static str> {
        args.next();

        let port = match args.next() {
            Some(port) => port,
            None => return Err("Nenhuma porta especificada"),
        };

        let port = port
            .parse()
            .expect("A porta deve ser um inteiro unsigned de 16 bits");

        Ok(ServerConfig { port })
    }
}
