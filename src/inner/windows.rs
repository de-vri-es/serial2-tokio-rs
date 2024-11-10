use std::io::{IoSliceMut, IoSlice};
use std::mem::ManuallyDrop;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::pin::Pin;
use std::task::Poll;
use tokio::net::windows::named_pipe::NamedPipeClient;

pub struct SerialPort {
	io: NamedPipeClient,
}

impl SerialPort {
	pub fn wrap(mut inner: serial2::SerialPort) -> std::io::Result<Self> {
		// We don't want timeouts on the operations themselves.
		// The user can use `tokio::time::timeout()` if they want.
		inner.set_read_timeout(std::time::Duration::from_millis(u32::MAX.into()))?;
		inner.set_write_timeout(std::time::Duration::from_millis(u32::MAX.into()))?;

		// First try to convert the inner serial port to a `NamedPipeClient`.
		// Only when that succeeded relinquish ownership of the file handle by forggeting `inner`.
		let io = unsafe { NamedPipeClient::from_raw_handle(inner.as_raw_handle())? };
		std::mem::forget(inner);

		Ok(Self {
			io,
		})
	}

	pub fn try_clone(&self) -> std::io::Result<Self> {
		Self::wrap(self.with_raw(|raw| raw.try_clone())?)
	}

	pub fn with_raw<F, R>(&self, function: F) -> R
	where
		F: FnOnce(&serial2::SerialPort) -> R
	{
		let serial_port = ManuallyDrop::new(unsafe {
			serial2::SerialPort::from_raw_handle(self.io.as_raw_handle())
		});
		function(&serial_port)
	}

	pub fn with_raw_mut<F, R>(&mut self, function: F) -> R
	where
		F: FnOnce(&mut serial2::SerialPort) -> R
	{
		let mut serial_port = ManuallyDrop::new(unsafe {
			serial2::SerialPort::from_raw_handle(self.io.as_raw_handle())
		});
		function(&mut serial_port)
	}

	pub async fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
		loop {
			self.io.readable().await?;
			match self.io.try_read(buf) {
				Ok(n) => return Ok(n),
				Err(e) => {
					if e.kind() == std::io::ErrorKind::WouldBlock {
						continue
					} else {
						return Err(e)
					}
				}
			}
		}
	}

	pub async fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
		if bufs.is_empty() {
			self.read(&mut []).await
		} else {
			self.read(&mut bufs[0]).await
		}
	}

	pub fn is_read_vectored(&self) -> bool {
		false
	}

	pub async fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
		loop {
			self.io.writable().await?;
			match self.io.try_write(buf) {
				Ok(n) => return Ok(n),
				Err(e) => {
					if e.kind() == std::io::ErrorKind::WouldBlock {
						continue
					} else {
						return Err(e)
					}
				}
			}
		}
	}

	pub async fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> std::io::Result<usize> {
		if bufs.is_empty() {
			self.write(&[]).await
		} else {
			self.write(&bufs[0]).await
		}
	}

	pub fn is_write_vectored(&self) -> bool {
		false
	}

	pub fn poll_read(
		&mut self,
		cx: &mut std::task::Context<'_>,
		buf: &mut tokio::io::ReadBuf<'_>,
	) -> Poll<std::io::Result<()>> {
		tokio::io::AsyncRead::poll_read(Pin::new(&mut self.io), cx, buf)
	}

	pub fn poll_write(
		&mut self,
		cx: &mut std::task::Context<'_>,
		buf: &[u8],
	) -> Poll<std::io::Result<usize>> {
		tokio::io::AsyncWrite::poll_write(Pin::new(&mut self.io), cx, buf)
	}

	pub fn poll_write_vectored(
		&mut self,
		cx: &mut std::task::Context<'_>,
		bufs: &[IoSlice<'_>],
	) -> Poll<Result<usize, std::io::Error>> {
		if bufs.is_empty() {
			self.poll_write(cx, &[])
		} else {
			self.poll_write(cx, &bufs[0])
		}
	}

	pub fn poll_shutdown(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), std::io::Error>> {
		// Serial ports can not be shut down.
		let error = winapi::shared::winerror::WSAENOTSOCK;
		Poll::Ready(Err(std::io::Error::from_raw_os_error(error as i32)))
	}
}

impl std::fmt::Debug for SerialPort {
	#[inline]
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.with_raw(|serial_port| {
			std::fmt::Debug::fmt(serial_port, f)
		})
	}
}
