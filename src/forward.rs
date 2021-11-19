use log::debug;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};
use std::net::{TcpListener, TcpStream};
use std::thread::spawn;

/// TcpProxy runs one thread looping to accept new connections
/// and then two separate threads per connection for writing to each end
pub struct TcpProxyClient {
    /// The handle for the outer thread, accepting new connections
    pub forward_thread: std::thread::JoinHandle<()>,
}

impl TcpProxyClient {
    /// Create a new TCP proxy, binding to listen_port and forwarding and receiving traffic from
    /// proxy_to
    pub fn new(
        listen_port: u16,
        proxy_to: String,
        local_only: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let ip = if local_only {
            Ipv4Addr::LOCALHOST
        } else {
            Ipv4Addr::UNSPECIFIED
        };
        let listener_forward = TcpListener::bind(SocketAddr::new(IpAddr::V4(ip), listen_port))?;

        let forward_thread = spawn(move || {
            let mut connect_cnt = 0_usize;
            loop {
                let (stream_forward, addr) = listener_forward
                    .accept()
                    .expect("Failed to accept connection");
                debug!("New connection");
                println!("Connection from {}", addr);
                connect_cnt += 1;

                //let mut sender_forward = TcpStream::connect(proxy_to).expect("Failed to bind");
                //let sender_backward = sender_forward.try_clone().expect("Failed to clone stream");
                let mut stream_backward =
                    stream_forward.try_clone().expect("Failed to clone stream");
                let connect_path = format!("{}/connection{}", proxy_to, connect_cnt);
                let upload_path = format!("{}/upload",connect_path);
                let download_path = format!("{}/download",connect_path);
                std::fs::create_dir(&connect_path)
                    .expect("Fail to create the folder for connection");
                std::fs::create_dir(&upload_path)
                    .expect("Fail to create the upload folder for connection");
                std::fs::create_dir(&download_path)
                    .expect("Fail to create the download folder for connection");

                let _upload_thread = spawn(move || {
                    || -> () {
                        let mut send_cnt = 0_usize;
                        let mut stream_forward = BufReader::new(stream_forward);
                        loop {
                            send_cnt += 1;
                            let length = {
                                let buffer = stream_forward.fill_buf().unwrap();
                                let length = buffer.len();
                                if buffer.is_empty() {
                                    // Connection closed
                                    debug!("Client closed connection");
                                    std::fs::File::create(format!(
                                        "{}/{}.ftcp",
                                        upload_path, send_cnt+1
                                    )).expect("Fail to create the end flag.");
                                    return;
                                }
                                let mut dst = std::fs::File::create(format!(
                                    "{}/{}.ftcp",
                                    upload_path, send_cnt
                                ))
                                .expect("Fail to create the pack file");
                                dst.write_all(&buffer).expect("Failed to write to file");
                                dst.flush().expect("Failed to flush file");

                                std::fs::File::create(format!(
                                    "{}/{}.lock",
                                    upload_path, send_cnt
                                ))
                                .expect("Fail to create the lock file");
                                //sender_forward
                                //    .write_all(&buffer)
                                //    .expect("Failed to write to remote");
                                //sender_forward.flush().expect("Failed to flush remote");
                                length
                            };
                            stream_forward.consume(length);
                        }
                    }();
                    std::fs::remove_dir_all(upload_path).unwrap_or_default();
                });

                let _download_thread = spawn(move || {
                    || -> () {
                        let mut recv_cnt = 0_usize;
                        loop {
                            recv_cnt += 1;
                            let length = {
                                let path;
                                loop {
                                    if let Ok(p) = std::fs::OpenOptions::new()
                                        .read(true)
                                        .open(format!("{}/{}.ftcp", download_path, recv_cnt))
                                    {
                                        path = p;
                                        break;
                                    }
                                }
                                let buffer: Vec<_> = path.bytes().map(|ch| ch.unwrap()).collect(); //.fill_buf().unwrap();
                                let length = buffer.len();
                                if buffer.is_empty() {
                                    // Connection closed
                                    debug!("Remote closed connection");
                                    return;
                                }
                                if stream_backward.write_all(&buffer).is_err() {
                                    // Connection closed
                                    debug!("Client closed connection");
                                    return;
                                }

                                stream_backward.flush().expect("Failed to flush locally");
                                length
                            };
                            //println!("{} bytes downloaded.", length);
                            //sender_backward.consume(length);
                        }
                    }();

                    std::fs::remove_dir_all(download_path).unwrap_or_default();
                });
            }
        });

        Ok(Self { forward_thread })
    }
}







pub struct TcpProxyServer {
    /// The handle for the outer thread, accepting new connections
    pub forward_thread: std::thread::JoinHandle<()>,
}

impl TcpProxyServer {
    /// Create a new TCP proxy, binding to listen_port and forwarding and receiving traffic from
    /// proxy_to
    pub fn new(
        listen_folder: String,
        proxy_to: String
    ) -> Result<Self, Box<dyn std::error::Error>> {

        let forward_thread = spawn(move || {
            let mut connect_cnt=0_usize;
            loop {
                connect_cnt+=1;
                let connect_path=format!("{}/connection{}",listen_folder,connect_cnt);
                loop
                {
                    if std::fs::read_dir(&connect_path).is_ok()
                    {break;}
                }
                let upload_path=format!("{}/upload",connect_path);
                let download_path=format!("{}/download",connect_path);

                let mut sender_forward = TcpStream::connect(&proxy_to).expect("Failed to bind");
                let sender_backward = sender_forward.try_clone().expect("Failed to clone stream");

                let _upload_thread=spawn(move || {
                    //let mut stream_forward = BufReader::new(stream_forward);
                    let mut upload_cnt=0_usize;
                    loop {
                        upload_cnt+=1;
                        let path;
                        loop
                        {
                            if std::fs::File::open(&format!("{}/{}.lock",upload_path,upload_cnt)).is_ok()
                            {
                                path=std::fs::File::open(&format!("{}/{}.ftcp",upload_path,upload_cnt)).unwrap();
                                break;
                            }
                        }
                        let length = {
                            let buffer: Vec<_> = path.bytes().map(|ch| ch.unwrap()).collect();;
                            let length = buffer.len();
                            if buffer.is_empty() {
                                // Connection closed
                                debug!("Client closed connection");
                                return;
                            }
                            sender_forward
                                .write_all(&buffer)
                                .expect("Failed to write to http proxy");
                            sender_forward.flush().expect("Failed to flush http proxy");
                            length
                        };
                        //stream_forward.consume(length);
                    }
                });

                let _download_thread = spawn(move || {
                    let mut sender_backward = BufReader::new(sender_backward);
                    let mut recv_cnt=0_usize;
                    loop {
                        recv_cnt+=1;
                        let length = {
                            let buffer = sender_backward.fill_buf().unwrap();
                            let length = buffer.len();
                            if buffer.is_empty() {
                                // Connection closed
                                debug!("Remote closed connection");
                                return;
                            }

                            let mut dst = std::fs::File::create(format!(
                                "{}/{}.ftcp",
                                download_path, recv_cnt
                            ))
                            .expect("Fail to create the pack file");
                            dst.write_all(&buffer).expect("Failed to write to file");
                            dst.flush().expect("Failed to flush file");

                            std::fs::File::create(format!(
                                "{}/{}.lock",
                                download_path, recv_cnt
                            ))
                            .expect("Fail to create the lock file");

                            // if stream_backward.write_all(&buffer).is_err() {
                            //     // Connection closed
                            //     debug!("Client closed connection");
                            //     return;
                            // }

                            // stream_backward.flush().expect("Failed to flush locally");
                            length
                        };
                        sender_backward.consume(length);
                    }
                });
            }
        });

        Ok(Self { forward_thread })
    }
}
