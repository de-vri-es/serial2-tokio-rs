use tokio::io::{AsyncReadExt, AsyncWriteExt};

use serial2_tokio::SerialPort;

#[tokio::main(flavor = "current_thread")]
async fn main() {
	if let Err(()) = do_main().await {
		std::process::exit(1);
	}
}

async fn do_main() -> Result<(), ()> {
	let args: Vec<_> = std::env::args().collect();
	if args.len() != 3 {
		let prog_name = args[0].rsplit_once('/').map(|(_parent, name)| name).unwrap_or(&args[0]);
		eprintln!("Usage: {} PORT BAUD", prog_name);
		return Err(());
	}

	let port_name = &args[1];
	let baud_rate: u32 = args[2]
		.parse()
		.map_err(|_| eprintln!("Error: invalid baud rate: {}", args[2]))?;

	let port = SerialPort::open(port_name, baud_rate)
		.map_err(|e| eprintln!("Error: Failed to open {}: {}", port_name, e))?;

	tokio::try_join!(
		read_stdin_loop(&port, port_name),
		read_serial_loop(&port, port_name),
	)?;
	Ok(())
}

async fn read_stdin_loop(port: &SerialPort, port_name: &str) -> Result<(), ()> {
	let mut stdin = tokio::io::stdin();
	let mut buffer = [0; 512];
	loop {
		let read = stdin
			.read(&mut buffer)
			.await
			.map_err(|e| eprintln!("Error: Failed to read from stdin: {}", e))?;
		if read == 0 {
			return Ok(());
		} else {
			port.write(&buffer[..read])
				.await
				.map_err(|e| eprintln!("Error: Failed to write to {}: {}", port_name, e))?;
		}
	}
}

async fn read_serial_loop(port: &SerialPort, port_name: &str) -> Result<(), ()> {
	let mut stdout = tokio::io::stdout();
	let mut buffer = [0; 512];
	loop {
		match port.read(&mut buffer).await {
			Ok(0) => return Ok(()),
			Ok(n) => {
				stdout
					.write_all(&buffer[..n])
					.await
					.map_err(|e| eprintln!("Error: Failed to write to stdout: {}", e))?;
			},
			Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
			Err(e) => {
				eprintln!("Error: Failed to read from {}: {}", port_name, e);
				return Err(());
			},
		}
	}
}
