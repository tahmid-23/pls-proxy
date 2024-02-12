use std::error::Error;
use base64::Engine;
use clap::Parser;
use http_body_util::Empty;
use hyper::body::{Body, Bytes};
use hyper::Request;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use log4rs::append::console::ConsoleAppender;
use log4rs::Config;
use log4rs::config::{Appender, Root};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use log::{info, LevelFilter, warn};
use log4rs::encode::pattern::PatternEncoder;

#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Local proxy port
    #[arg(short = 'P', long)]
    port: u16,

    /// Remote server address
    #[arg(long = "by")]
    server_address: String,

    /// Proxy username
    #[arg(short, long, requires = "password")]
    username: Option<String>,

    /// Proxy password
    #[arg(short, long, requires = "username")]
    password: Option<String>,

    /// Target address
    target: String
}

async fn connect_to_proxy<A, B>(connect_request: Request<B>, address: A) -> Result<TokioIo<Upgraded>, Box<dyn Error + Send + Sync>>
    where A: ToSocketAddrs,
          B: Body + Send + 'static,
          B::Data: Send,
          B::Error: Into<Box<dyn Error + Send + Sync>>, {
    let proxy_stream = TcpStream::connect(address).await?;
    if let Err(e) = proxy_stream.set_nodelay(true) {
        warn!("Failed to set TCP_NODELAY: {}", e);
    }
    let proxy_stream = TokioIo::new(proxy_stream);
    let (mut proxy_sender, proxy_connection) = hyper::client::conn::http1::handshake(proxy_stream).await?;
    tokio::spawn(proxy_connection.with_upgrades());

    let connect_response = proxy_sender.send_request(connect_request).await?;
    let target_stream = hyper::upgrade::on(connect_response).await?;
    Ok(TokioIo::new(target_stream))
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    log4rs::init_config(
        Config::builder()
            .appender(
                Appender::builder().build(
                    "console",
                    Box::new(
                        ConsoleAppender::builder()
                            .encoder(
                                Box::new(
                                    PatternEncoder::new("{h({l})}: {m}{n}")
                                )
                            )
                            .build()
                    )
                )
            )
            .build(Root::builder().appender("console").build(LevelFilter::Info))
            .unwrap()
    ).unwrap();

    let mut connect_request_builder = Request::connect(cli.target.clone())
        .header(hyper::header::HOST, cli.target);
    if let Some(proxy_username) = cli.username {
        let proxy_password = cli.password.unwrap();

        let encoded_credentials = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", proxy_username, proxy_password));
        connect_request_builder = connect_request_builder
            .header(hyper::header::PROXY_AUTHORIZATION, format!("Basic {}", encoded_credentials));
    }
    let connect_request = connect_request_builder
        .body(Empty::<Bytes>::new())
        .unwrap_or_else(|e| panic!("Failed to create HTTP CONNECT request template: {}", e));

    let listener = TcpListener::bind(("127.0.0.1", cli.port)).await
        .unwrap_or_else(|e| panic!("Failed to bind to 127.0.0.1:{}, reason: {}", cli.port, e));

    info!("Proxy started on 127.0.0.1:{}", cli.port);

    loop {
        let (mut socket, from_address) = listener.accept().await.expect("Failed to accept connection");

        info!("Accepted connection from {}", from_address);

        let connect_request = connect_request.clone();
        let server_address = cli.server_address.clone();
        tokio::spawn(async move {
            info!("Initializing proxy connection");
            match connect_to_proxy(connect_request, server_address).await {
                Ok(mut proxy_stream) => {
                    info!("Successfully connected to proxy");

                    match tokio::io::copy_bidirectional(&mut socket, &mut proxy_stream).await {
                        Ok(_) => {
                            info!("Finished connection from {}", from_address);
                        }
                        Err(e) => {
                            warn!("Error while proxying {}: {}", from_address, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to connect to proxy: {}", e);
                }
            }
        });
    }
}
