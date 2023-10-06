use std::io::{IoSliceMut, IoSlice};
use std::os::fd::AsRawFd;
use std::task::{ready, Poll};
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;

pub struct SerialPort {
	io: AsyncFd<serial2::SerialPort>,
}

impl SerialPort {
	pub fn wrap(inner: serial2::SerialPort) -> std::io::Result<Self> {
		Ok(Self {
			io: AsyncFd::new(inner)?,
		})
	}

	pub fn with_raw<F, R>(&self, function: F) -> R
	where
		F: FnOnce(&serial2::SerialPort) -> R
	{
		function(self.io.get_ref())
	}

	pub fn with_raw_mut<F, R>(&mut self, function: F) -> R
	where
		F: FnOnce(&mut serial2::SerialPort) -> R
	{
		function(self.io.get_mut())
	}

	pub async fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
		self.io.async_io(Interest::READABLE, |inner| {
			unsafe {
				check_ret(libc::read(inner.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()))
			}
		}).await
	}

	pub async fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
		self.io.async_io(Interest::READABLE, |inner| {
			unsafe {
				let buf_count = i32::try_from(bufs.len()).unwrap_or(i32::MAX);
				check_ret(libc::readv(inner.as_raw_fd(), bufs.as_mut_ptr().cast(), buf_count))
			}
		}).await
	}

	pub fn is_read_vectored(&self) -> bool {
		true
	}

	pub async fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
		self.io.async_io(Interest::WRITABLE, |inner| {
			unsafe {
				check_ret(libc::write(inner.as_raw_fd(), buf.as_ptr().cast(), buf.len()))
			}
		}).await
	}

	pub async fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> std::io::Result<usize> {
		self.io.async_io(Interest::WRITABLE, |inner| {
			unsafe {
				let buf_count = i32::try_from(bufs.len()).unwrap_or(i32::MAX);
				check_ret(libc::writev(inner.as_raw_fd(), bufs.as_ptr().cast(), buf_count))
			}
		}).await
	}

	pub fn is_write_vectored(&self) -> bool {
		true
	}

	pub fn poll_read(
		&mut self,
		cx: &mut std::task::Context<'_>,
		buf: &mut tokio::io::ReadBuf<'_>,
	) -> Poll<std::io::Result<()>> {
		loop {
			let mut guard = ready!(self.io.poll_read_ready(cx)?);
			let result = guard.try_io(|inner|{
				unsafe {
					let unfilled = buf.unfilled_mut();
					check_ret(libc::read(inner.as_raw_fd(), unfilled.as_mut_ptr().cast(), unfilled.len()))
				}
			});
			match result {
				Ok(result) => {
					let read = result?;
					unsafe { buf.assume_init(read) };
					buf.advance(read);
					return Poll::Ready(Ok(()));
				},
				Err(_would_block) => continue,
			}
		}
	}

	pub fn poll_write(
		&mut self,
		cx: &mut std::task::Context<'_>,
		buf: &[u8],
	) -> Poll<std::io::Result<usize>> {
		loop {
			let mut guard = ready!(self.io.poll_read_ready(cx)?);
			let result = guard.try_io(|inner|{
				check_ret(unsafe {
					libc::write(inner.as_raw_fd(), buf.as_ptr().cast(), buf.len())
				})
			});
			match result {
				Ok(result) => return Poll::Ready(result),
				Err(_would_block) => continue,
			}
		}
	}

	pub fn poll_write_vectored(
		&mut self,
		cx: &mut std::task::Context<'_>,
		bufs: &[IoSlice<'_>],
	) -> Poll<Result<usize, std::io::Error>> {
		loop {
			let mut guard = ready!(self.io.poll_read_ready(cx)?);
			let result = guard.try_io(|inner| {
				let buf_count = i32::try_from(bufs.len()).unwrap_or(i32::MAX);
				check_ret(unsafe {
					libc::writev(inner.as_raw_fd(), bufs.as_ptr().cast(), buf_count)
				})
			});
			match result {
				Ok(result) => return Poll::Ready(result),
				Err(_would_block) => continue,
			}
		}
	}

	pub fn poll_shutdown(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), std::io::Error>> {
		// Serial ports can not be shut down.
		Poll::Ready(Err(std::io::Error::from_raw_os_error(libc::ENOTSOCK)))
	}
}

fn check_ret(value: isize) -> std::io::Result<usize> {
	if value < 0 {
		Err(std::io::Error::last_os_error())
	} else {
		Ok(value as usize)
	}
}
