use anyhow::Result;
use futures::prelude::*;
use smol::{Async, Task};
use std::io::Write;
use std::os::unix::net::UnixStream;

fn printer() -> Result<piper::Writer> {
    let (mut rx, tx) = piper::pipe(1024);

    Task::blocking(async move {
        let mut buf = [0u8; 1024];
        let mut stdout = std::io::stdout();

        loop {
            match rx.read(&mut buf).await {
                Ok(0) => {
                    println!("printer EOF");
                    break;
                }
                Ok(len) => {
                    stdout.write_all(&buf[..len]).unwrap();
                }
                Err(e) => {
                    println!("prinner error: {}", e);
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
        let mut buf: Vec<_> = std::iter::repeat(b'0').take(1023).collect();

        buf.push(b'\n');

        loop {
            if let Err(e) = tx.write_all(&buf).await {
                println!("streamer error: {}", e);
                break;
            }
        }
    })
    .detach();

    Ok(rx)
}

async fn connect() -> Result<()> {
    let mut socket = Async::<UnixStream>::connect("test.sock").await?;
    let mut printer = printer()?;
    let mut streamer = streamer()?;
    let mut socket_buf = [0u8; 1024];
    let mut streamer_buf = [0u8; 1024];

    loop {
        futures::select! {
            // whatever we read from `socket` we write it to `printer`
            res = socket.read(&mut socket_buf).fuse() => match res? {
                0 => break,
                len => printer.write_all(&socket_buf[..len]).await?,
            },

            // whatever we read from `streamer` we write it to `socket`
            res = streamer.read(&mut streamer_buf).fuse() => match res? {
                0 => break,
                len => socket.write_all(&streamer_buf[..len]).await?,
            }
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    smol::run(async { connect().await })
}
