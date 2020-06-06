use anyhow::Result;
use futures::prelude::*;
use smol::{Async, Task};
use std::os::unix::net::{UnixListener, UnixStream};

fn blackhole() -> Result<piper::Writer> {
    let (mut rx, tx) = piper::pipe(1024);

    Task::spawn(async move {
        let mut buf = [0u8; 1024];

        loop {
            match rx.read(&mut buf).await {
                Ok(0) => {
                    println!("blackhole EOF");
                    break;
                }
                Ok(_len) => {}
                Err(e) => {
                    println!("blackhole error: {}", e);
                    break;
                }
            }
        }
    })
    .detach();

    Ok(tx)
}

fn streamer() -> Result<piper::Reader> {
    let (rx, mut tx) = piper::pipe(1024);

    Task::spawn(async move {
        let mut buf = [0u8; 1024];

        'outer: loop {
            for ch in b'A'..b'Z' {
                for i in 0..buf.len() - 1 {
                    buf[i] = ch
                }

                buf[buf.len() - 1] = b'\n';

                if let Err(e) = tx.write_all(&buf).await {
                    println!("streamer error: {}", e);
                    break 'outer;
                }
            }
        }
    })
    .detach();

    Ok(rx)
}

async fn handle_client(mut client: Async<UnixStream>) -> Result<()> {
    let mut blackhole = blackhole()?;
    let mut streamer = streamer()?;
    let mut client_buf = [0u8; 1024];
    let mut streamer_buf = [0u8; 1024];

    loop {
        futures::select! {
            // whatever we read from `client` we write it to `blackhole`
            res = client.read(&mut client_buf).fuse() => match res? {
                0 => break,
                len => blackhole.write_all(&client_buf[..len]).await?,
            },

            // whatever we read from `streamer` we write it to `client`
            res = streamer.read(&mut streamer_buf).fuse() => match res? {
                0 => break,
                len => client.write_all(&streamer_buf[..len]).await?,
            }
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    smol::run(async {
        let _ = std::fs::remove_file("test.sock");
        let listener = Async::<UnixListener>::bind("test.sock")?;
        let mut incoming = listener.incoming();

        while let Some(Ok(client)) = incoming.next().await {
            Task::spawn(async move {
                if let Err(e) = handle_client(client).await {
                    println!("handle_client error: {}", e);
                }
            })
            .detach();
        }

        println!("Hello, world!");

        Ok(())
    })
}
