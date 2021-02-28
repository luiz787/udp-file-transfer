use std::env;
use std::ffi::OsStr;
use std::net::IpAddr;
use std::path::Path;
use std::str;

pub struct ClientConfig {
    pub ip: IpAddr,
    pub port: u16,
    pub filename: Filename,
}

pub struct Filename {
    pub filename: String,
}

impl Filename {
    pub fn new(filename: Option<String>) -> Result<Filename, &'static str> {
        let filename = match filename {
            Some(filename) => filename,
            None => return Err("Nome do arquivo não especificado"),
        };

        if filename.len() > 15 {
            return Err("Nome não permitido");
        }
        if !filename.contains(".") {
            return Err("Nome não permitido");
        }
        if filename.matches(".").count() > 1 {
            return Err("Nome não permitido");
        }

        let extension = Path::new(&filename)
            .extension()
            .and_then(OsStr::to_str)
            .unwrap();
        if extension.len() > 3 {
            return Err("Nome não permitido");
        }

        if !filename.chars().all(|ch| ch.is_ascii()) {
            return Err("Nome não permitido");
        }

        return Ok(Filename { filename });
    }
}

impl ClientConfig {
    pub fn new(mut args: env::Args) -> Result<ClientConfig, &'static str> {
        args.next();

        let ip = match args.next() {
            Some(ip) => ip,
            None => return Err("IP do servidor não especificado."),
        };

        let port = match args.next() {
            Some(port) => port,
            None => return Err("Porta do servidor não especificada."),
        };

        // TODO: refactor to return Err instead of panic
        let port = port
            .parse()
            .expect("A porta deve ser um inteiro unsigned de 16 bits");

        let ip = ip.parse::<IpAddr>();

        let ip = match ip {
            Ok(ip) => ip,
            Err(_error) => return Err("Falha ao fazer o parse do endereço ip"),
        };

        let filename = Filename::new(args.next());
        let filename = match filename {
            Ok(filename) => filename,
            Err(msg) => return Err(msg),
        };

        Ok(ClientConfig { ip, port, filename })
    }
}
