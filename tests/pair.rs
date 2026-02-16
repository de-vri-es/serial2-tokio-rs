#![cfg(unix)]

use assert2::{assert};
use serial2_tokio::SerialPort;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn open_pair() {
	assert!(let Ok((mut a, mut b)) = SerialPort::pair());
	assert!(let Ok(()) = a.write_all(b"Hello!").await);
	let mut buffer = [0; 6];
	assert!(let Ok(6) = b.read_exact(&mut buffer).await);
	assert!(&buffer == b"Hello!");

	assert!(let Ok(()) = b.write_all(b"Goodbye!").await);
	let mut buffer = [0; 8];
	assert!(let Ok(8) = a.read_exact(&mut buffer).await);
	assert!(&buffer == b"Goodbye!");
}
