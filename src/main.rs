use anyhow::Result;
use colored::*;
use dashmap::DashMap;
use std::net::SocketAddr;
use std::ops::Range;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpListener;
use tokio::select;

const CLIENT_NAME_RANGE: Range<usize> = 3..16;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let server_socket_addr = "192.168.1.200:30000";
    let listener = TcpListener::bind(server_socket_addr).await?;
    let (message_sender, receiver) = tokio::sync::broadcast::channel::<(String, SocketAddr)>(32);
    drop(receiver);

    let client_names = Arc::new(DashMap::<SocketAddr, String>::new());

    loop {
        let (mut stream, client_socket_addr) = listener.accept().await?;
        tracing::info!("Client have connected");

        let mut client_message_receiver = message_sender.subscribe();
        let client_message_sender = message_sender.clone();
        let client_names = client_names.clone();

        tokio::spawn(async move {
            let (stream_reader, stream_writer) = stream.split();

            let mut stream_buf_writer = BufWriter::new(stream_writer);
            let mut stream_buf_reader = BufReader::new(stream_reader);
            let mut client_input = String::new();

            loop {
                client_input.clear();
                stream_buf_writer
                    .write_all("Name: ".as_bytes())
                    .await
                    .unwrap();
                stream_buf_writer.flush().await.unwrap();
                stream_buf_reader
                    .read_line(&mut client_input)
                    .await
                    .unwrap();

                if !CLIENT_NAME_RANGE.contains(&client_input.trim().len()) {
                    stream_buf_writer
                        .write_all(
                            format!("The name length should be in the range {CLIENT_NAME_RANGE:?}, please try again!\n")
                                .red()
                                .as_bytes(),
                        )
                        .await
                        .unwrap();
                    stream_buf_writer.flush().await.unwrap();
                    continue;
                }

                if client_input
                    .trim()
                    .find(|char: char| !(char.is_alphanumeric()))
                    .is_some()
                {
                    stream_buf_writer
                        .write_all(
                            "The name is invalid, only alphanumeric characters are allowed\n"
                                .red()
                                .as_bytes(),
                        )
                        .await
                        .unwrap();
                    stream_buf_writer.flush().await.unwrap();
                    continue;
                }

                if client_names
                    .iter()
                    .find(|entry| client_input.trim().eq(entry.value()))
                    .is_none()
                {
                    break;
                } else {
                    stream_buf_writer
                        .write_all(
                            "The name is taken, please try again with another name!\n"
                                .red()
                                .as_bytes(),
                        )
                        .await
                        .unwrap();
                    stream_buf_writer.flush().await.unwrap();
                }
            }

            client_message_sender
                .send((
                    format!("{} have connected!\n", client_input.trim())
                        .bright_red()
                        .to_string(),
                    client_socket_addr,
                ))
                .unwrap();
            _ = client_names.insert(client_socket_addr, client_input.trim().to_string());

            client_input.clear();

            loop {
                select! {
                    _ = stream_buf_reader
                    .read_line(&mut client_input) => {
                        _ = client_message_sender.send((client_input.trim().to_string(), client_socket_addr));
                        client_input.clear();
                    },
                    Ok((message, message_client_socket_addr)) = client_message_receiver.recv() => {
                        if message_client_socket_addr != client_socket_addr {
                            let formatted_message = {
                                let client_name = client_names.get(&message_client_socket_addr).unwrap();
                                format!("{}: {}\n", client_name.bright_blue(), message.cyan())
                            };
                            stream_buf_writer.write_all(formatted_message.as_bytes()).await.unwrap();
                            stream_buf_writer.flush().await.unwrap();
                        }
                    },
                }
            }
        });
    }
}
