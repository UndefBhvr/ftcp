//use std::convert::*;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};
//use std::path::*;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
//use tokio::sync::*;
use tokio::task::spawn;

// use core::future::*;
// use core::pin::*;
// use core::task::*;
use core::time::Duration;

/*TODO

struct FileWaiter {
    path: String,
    waker: Option<Waker>,
    result: OnceCell<tokio::fs::File>,
}

impl Future for FileWaiter {
    type Output = tokio::fs::File;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(p) = self.result.take() {
            Poll::Ready(p)
        } else {
            self.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl FileWaiter {
    fn new(p: String) -> Self {
        tokio::task::spawn(async {
            loop {
                tokio::time::sleep(Duration::from_millis(15)).await;
                if let Ok(p) = tokio::fs::OpenOptions::new().read(true).open(p).await {
                    path = p;
                    break;
                }
            }
        });
        Self {
            path: p,
            waker: None,
            result: OnceCell::new(),
        }
    }
}

*/

async fn wait_for_file_locked(path: &str, lock: &str) -> tokio::fs::File {
    let ret;
    loop {
        if tokio::fs::File::open(&lock).await.is_ok() {
            ret = tokio::fs::File::open(&path).await.unwrap();
            break;
        }
        tokio::time::sleep(Duration::from_millis(3)).await;
    }
    ret
}

/// TcpProxy runs one thread looping to accept new connections
/// and then two separate threads per connection for writing to each end
pub struct FtcpProxyClient {
    /// The handle for the outer thread, accepting new connections
    pub forward_thread: tokio::task::JoinHandle<()>,
}

impl FtcpProxyClient {
    /// Create a new FTCP proxy client, binding to listen_port and forwarding and receiving traffic from
    /// the proxy_to, which must be a folder
    pub async fn new(
        listen_port: u16,
        proxy_to: String,
        local_only: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let ip = if local_only {
            Ipv4Addr::LOCALHOST
        } else {
            Ipv4Addr::UNSPECIFIED
        };
        let listener_forward =
            TcpListener::bind(SocketAddr::new(IpAddr::V4(ip), listen_port)).await?;

        let forward_thread = spawn(async move {
            let mut connect_cnt = 0_usize;
            loop {
                let (stream_forward, addr) = listener_forward
                    .accept()
                    .await
                    .expect("Failed to accept connection");
                //debug!("New connection");
                println!("Connection from {}", addr);
                connect_cnt += 1;

                let stream_forward = stream_forward.into_std().unwrap();
                let mut stream_backward =
                    TcpStream::from_std(stream_forward.try_clone().unwrap()).unwrap();
                let stream_forward = TcpStream::from_std(stream_forward).unwrap();

                let connect_path = format!("{}/connection{}", proxy_to, connect_cnt);
                let upload_path = format!("{}/upload", connect_path);
                let download_path = format!("{}/download", connect_path);
                std::fs::create_dir(&connect_path)
                    .expect("Fail to create the folder for connection");
                std::fs::create_dir(&upload_path)
                    .expect("Fail to create the upload folder for connection");
                std::fs::create_dir(&download_path)
                    .expect("Fail to create the download folder for connection");

                let _upload_thread = spawn(async move {
                    let mut send_cnt = 0_usize;
                    let mut stream_forward = BufReader::new(stream_forward);
                    loop {
                        send_cnt += 1;
                        let length = {
                            let buffer = stream_forward.fill_buf().await.expect("Fail to read");
                            let length = buffer.len();
                            if buffer.is_empty() {
                                // Connection closed
                                //debug!("Client closed connection");
                                tokio::fs::File::create(format!(
                                    "{}/{}.ftcp",
                                    upload_path,
                                    send_cnt + 1
                                ))
                                .await
                                .expect("Fail to create the end flag.");
                                break;
                            }
                            let mut dst = tokio::fs::File::create(format!(
                                "{}/{}.ftcp",
                                upload_path, send_cnt
                            ))
                            .await
                            .expect("Fail to create the pack file");
                            dst.write_all(&buffer)
                                .await
                                .expect("Failed to write to file");
                            dst.flush().await.expect("Failed to flush file");

                            tokio::fs::File::create(format!("{}/{}.lock", upload_path, send_cnt))
                                .await
                                .expect("Fail to create the lock file");
                            length
                        };
                        stream_forward.consume(length);
                    }
                });

                let _download_thread = spawn(async move {
                    let mut recv_cnt = 0_usize;
                    loop {
                        recv_cnt += 1;
                        let _length = {
                            let path = wait_for_file_locked(
                                &format!("{}/{}.ftcp", download_path, recv_cnt),
                                &format!("{}/{}.lock", download_path, recv_cnt),
                            )
                            .await;
                            let mut buffer = tokio::io::BufReader::new(path);
                            let buffer = buffer.fill_buf().await.expect("Fail to read");
                            let length = buffer.len();
                            if buffer.is_empty() {
                                // Connection closed
                                //debug!("Remote closed connection");
                                break;
                            }
                            if stream_backward.write_all(&buffer).await.is_err() {
                                // Connection closed
                                //debug!("Client closed connection");
                                break;
                            }

                            stream_backward
                                .flush()
                                .await
                                .expect("Failed to flush locally");
                            length
                        };
                        //println!("{} bytes downloaded.", length);
                        //stream_backward.consume(length);
                    }
                });
            }
        });

        Ok(Self { forward_thread })
    }
}

pub struct FtcpProxyServer {
    /// The handle for the outer thread, accepting new connections
    pub forward_thread: tokio::task::JoinHandle<()>,
}

impl FtcpProxyServer {
    /// Create a new TCP proxy server, binding to listen_folder and forwarding and receiving traffic from
    /// proxy_to, which must be a port for http proxy
    pub async fn new(
        listen_folder: String,
        proxy_to: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let forward_thread = spawn(async move {
            let mut connect_cnt = 0_usize;
            loop {
                connect_cnt += 1;
                let connect_path = format!("{}/connection{}", listen_folder, connect_cnt);
                loop {
                    if tokio::fs::read_dir(&connect_path).await.is_ok() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(3)).await;
                }
                let upload_path = format!("{}/upload", connect_path);
                let download_path = format!("{}/download", connect_path);

                let stream_forward = TcpStream::connect(&proxy_to).await.expect("Failed to bind");

                let stream_forward = stream_forward.into_std().unwrap();
                let stream_backward =
                    TcpStream::from_std(stream_forward.try_clone().unwrap()).unwrap();
                let mut stream_forward = TcpStream::from_std(stream_forward).unwrap();

                let _upload_thread = spawn(async move {
                    //let mut stream_forward = BufReader::new(stream_forward);
                    let mut upload_cnt = 0_usize;
                    loop {
                        upload_cnt += 1;
                        let path = wait_for_file_locked(
                            &format!("{}/{}.ftcp", upload_path, upload_cnt),
                            &format!("{}/{}.lock", upload_path, upload_cnt),
                        )
                        .await;
                        /*loop {
                            if tokio::fs::File::open(&format!(
                                "{}/{}.lock",
                                upload_path, upload_cnt
                            ))
                            .await
                            .is_ok()
                            {
                                path = tokio::fs::File::open(&format!(
                                    "{}/{}.ftcp",
                                    upload_path, upload_cnt
                                ))
                                .await
                                .unwrap();
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(20));
                        }*/
                        let _length = {
                            let mut buffer = BufReader::new(path);
                            let buffer = buffer.fill_buf().await.unwrap();
                            let length = buffer.len();
                            if buffer.is_empty() {
                                // Connection closed
                                //debug!("Client closed connection");
                                return;
                            }
                            stream_forward
                                .write_all(&buffer)
                                .await
                                .expect("Failed to write to http proxy");
                            stream_forward
                                .flush()
                                .await
                                .expect("Failed to flush http proxy");
                            length
                        };
                        //stream_forward.consume(length);
                    }
                });

                let _download_thread = spawn(async move {
                    let mut stream_backward = BufReader::new(stream_backward);
                    let mut recv_cnt = 0_usize;
                    loop {
                        recv_cnt += 1;
                        let length = {
                            let buffer = stream_backward.fill_buf().await.unwrap();
                            let length = buffer.len();
                            if buffer.is_empty() {
                                // Connection closed
                                //debug!("Remote closed connection");
                                return;
                            }

                            let mut dst = tokio::fs::File::create(&format!(
                                "{}/{}.ftcp",
                                download_path, recv_cnt
                            ))
                            .await
                            .expect("Fail to create the pack file");
                            dst.write_all(&buffer)
                                .await
                                .expect("Failed to write to file");
                            dst.flush().await.expect("Failed to flush file");

                            tokio::fs::File::create(&format!(
                                "{}/{}.lock",
                                download_path, recv_cnt
                            ))
                            .await
                            .expect("Fail to create the lock file");

                            // if stream_backward.write_all(&buffer).is_err() {
                            //     // Connection closed
                            //     debug!("Client closed connection");
                            //     return;
                            // }

                            // stream_backward.flush().expect("Failed to flush locally");
                            length
                        };
                        stream_backward.consume(length);
                    }
                });
            }
        });

        Ok(Self { forward_thread })
    }
}
