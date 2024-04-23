//! Serial port communication for [`tokio`] using [`serial2`].
//!
//! The `serial2-tokio` crate provides a cross-platform interface to serial ports.
//! It aims to provide a simpler interface than other alternatives.
//!
//! Currently supported features:
//! * Simple interface: one [`SerialPort`] struct for all supported platforms.
//! * List available ports.
//! * Custom baud rates on all supported platforms except Solaris and Illumos.
//! * Concurrent reads and writes from multiple tasks, even on Windows.
//! * Purge the OS buffers (useful to discard read noise when the line should have been silent, for example).
//! * Read and control individual modem status lines to use them as general purpose I/O.
//! * Cross platform configuration of serial port settings:
//!   * Baud rate
//!   * Character size
//!   * Stop bits
//!   * Parity checks
//!   * Flow control
//!   * Read/write timeouts
//!
//! You can open and configure a serial port in one go with [`SerialPort::open()`].
//! The second argument to `open()` must be a type that implements [`IntoSettings`].
//! In the simplest case, it is enough to pass a `u32` for the baud rate.
//! Doing that will also configure a character size of 8 bits with 1 stop bit and disables parity checks and flow control.
//! For full control over the applied settings, pass a closure that receives the the current [`Settings`] and return the desired settings.
//! If you do, you will almost always want to call [`Settings::set_raw()`] before changing any other settings.
//!
//! The [`SerialPort`] struct implements the standard [`tokio::io::AsyncRead`] and [`tokio::io::AsyncWrite`] traits,
//! as well as [`read()`][`SerialPort::read()`] and [`write()`][`SerialPort::write()`] functions that take `&self` instead of `&mut self`.
//! This allows you to use the serial port concurrently from multiple tasks.
//!
//! The [`SerialPort::available_ports()`] function can be used to get a list of available serial ports on supported platforms.
//!
//! # Example
//! This example opens a serial port and echoes back everything that is read.
//!
//! ```no_run
//! # async fn example() -> std::io::Result<()> {
//! use serial2_tokio::SerialPort;
//!
//! // On Windows, use something like "COM1" or "COM15".
//! let port = SerialPort::open("/dev/ttyUSB0", 115200)?;
//! let mut buffer = [0; 256];
//! loop {
//!     let read = port.read(&mut buffer).await?;
//!     port.write(&buffer[..read]).await?;
//! }
//! # }
//! ```

#![warn(missing_docs)]
#![warn(private_interfaces)]
#![warn(private_bounds)]

use std::io::{IoSliceMut, IoSlice};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::Poll;

mod inner;

pub use serial2::{
	COMMON_BAUD_RATES,
	CharSize,
	FlowControl,
	IntoSettings,
	KeepSettings,
	Parity,
	Settings,
	StopBits,
	TryFromError,
};

#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "rs4xx")))]
#[cfg(any(feature = "doc", feature = "rs4xx"))]
pub use serial2::rs4xx;

use tokio::io::{AsyncRead, AsyncWrite};

/// An asynchronous serial port for Tokio.
pub struct SerialPort {
	inner: inner::SerialPort,
}

impl SerialPort {
	/// Open and configure a serial port by path or name.
	///
	/// On Unix systems, the `name` parameter must be a path to a TTY device.
	/// On Windows, it must be the name of a COM device, such as COM1, COM2, etc.
	///
	/// The second argument is used to configure the serial port.
	/// For simple cases, you pass a `u32` for the baud rate.
	/// See [`IntoSettings`] for more information.
	///
	/// The library automatically uses the win32 device namespace on Windows, so COM ports above COM9 are supported out of the box.
	///
	/// # Example
	/// ```no_run
	/// # use serial2::SerialPort;
	/// # fn main() -> std::io::Result<()> {
	/// SerialPort::open("/dev/ttyUSB0", 115200)?;
	/// #   Ok(())
	/// # }
	/// ```
	pub fn open(path: impl AsRef<Path>, settings: impl IntoSettings) -> std::io::Result<Self> {
		let inner = serial2::SerialPort::open(path, settings)?;
		let inner = inner::SerialPort::wrap(inner)?;
		Ok(Self {
			inner,
		})
	}

	/// Get a list of available serial ports.
	///
	/// Not currently supported on all platforms.
	/// On unsupported platforms, this function always returns an error.
	pub fn available_ports() -> std::io::Result<Vec<PathBuf>> {
		serial2::SerialPort::available_ports()
	}

	/// Configure (or reconfigure) the serial port.
	pub fn set_configuration(&mut self, settings: &Settings) -> std::io::Result<()> {
		self.inner.with_raw_mut(|raw| raw.set_configuration(settings))
	}

	/// Get the current configuration of the serial port.
	///
	/// This function can fail if the underlying syscall fails,
	/// or if the serial port configuration can't be reported using [`Settings`].
	pub fn get_configuration(&self) -> std::io::Result<Settings> {
		self.inner.with_raw(|raw| raw.get_configuration())
	}

	/// Try to clone the serial port handle.
	///
	/// The cloned object refers to the same serial port.
	///
	/// Mixing reads and writes on different handles to the same serial port from different threads may lead to unexpect results.
	/// The data may end up interleaved in unpredictable ways.
	pub fn try_clone(&self) -> std::io::Result<Self> {
		let inner = self.inner.try_clone()?;
		Ok(Self { inner })
	}

	/// Read bytes from the serial port.
	///
	/// This is identical to [`AsyncReadExt::read()`][tokio::io::AsyncReadExt::read], except that this function takes a const reference `&self`.
	/// This allows you to use the serial port concurrently from multiple tasks.
	///
	/// Note that there are no guarantees about which task receives what data when multiple tasks are reading from the serial port.
	/// You should normally limit yourself to a single reading task and a single writing task.
	pub async fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
		self.inner.read(buf).await
	}

	/// Read bytes from the serial port into a slice of buffers.
	///
	/// Note that there are no guarantees about which task receives what data when multiple tasks are reading from the serial port.
	/// You should normally limit yourself to a single reading task and a single writing task.
	pub async fn read_vectored(&self, buf: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
		self.inner.read_vectored(buf).await
	}

	/// Check if the implementation supports vectored reads.
	///
	/// If this returns false, then [`Self::read_vectored()`] will only use the first buffer of the given slice.
	/// All platforms except for Windows support vectored reads.
	pub fn is_read_vectored(&self) -> bool {
		self.inner.is_read_vectored()
	}

	/// Write bytes to the serial port.
	///
	/// This is identical to [`AsyncWriteExt::write()`][tokio::io::AsyncWriteExt::write], except that this function takes a const reference `&self`.
	/// This allows you to use the serial port concurrently from multiple tasks.
	///
	/// Note that data written to the same serial port from multiple tasks may end up interleaved at the receiving side.
	/// You should normally limit yourself to a single reading task and a single writing task.
	pub async fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
		self.inner.write(buf).await
	}

	/// Write all bytes to the serial port.
	///
	/// This will continue to call [`Self::write()`] until the entire buffer has been written,
	/// or an I/O error occurs.
	///
	/// This is identical to [`AsyncWriteExt::write_all()`][tokio::io::AsyncWriteExt::write_all], except that this function takes a const reference `&self`.
	/// This allows you to use the serial port concurrently from multiple tasks.
	///
	/// Note that data written to the same serial port from multiple tasks may end up interleaved at the receiving side.
	/// You should normally limit yourself to a single reading task and a single writing task.
	pub async fn write_all(&self, buf: &[u8]) -> std::io::Result<()> {
		let mut written = 0;
		while written < buf.len() {
			written += self.write(&buf[written..]).await?;
		}
		Ok(())
	}

	/// Write bytes to the serial port from a slice of buffers.
	///
	/// This is identical to [`AsyncWriteExt::write_vectored()`][tokio::io::AsyncWriteExt::write_vectored], except that this function takes a const reference `&self`.
	/// This allows you to use the serial port concurrently from multiple tasks.
	///
	/// Note that data written to the same serial port from multiple tasks may end up interleaved at the receiving side.
	/// You should normally limit yourself to a single reading task and a single writing task.
	pub async fn write_vectored(&self, buf: &[IoSlice<'_>]) -> std::io::Result<usize> {
		self.inner.write_vectored(buf).await
	}

	/// Check if the implementation supports vectored writes.
	///
	/// If this returns false, then [`Self::write_vectored()`] will only use the first buffer of the given slice.
	/// All platforms except for Windows support vectored writes.
	pub fn is_write_vectored(&self) -> bool {
		self.inner.is_write_vectored()
	}

	/// Discard the kernel input and output buffers for the serial port.
	///
	/// When you write to a serial port, the data may be put in a buffer by the OS to be transmitted by the actual device later.
	/// Similarly, data received on the device can be put in a buffer by the OS untill you read it.
	/// This function clears both buffers: any untransmitted data and received but unread data is discarded by the OS.
	pub fn discard_buffers(&self) -> std::io::Result<()> {
		self.inner.with_raw(|raw| raw.discard_buffers())
	}

	/// Discard the kernel input buffers for the serial port.
	///
	/// Data received on the device can be put in a buffer by the OS untill you read it.
	/// This function clears that buffer: received but unread data is discarded by the OS.
	///
	/// This is particularly useful when communicating with a device that only responds to commands that you send to it.
	/// If you discard the input buffer before sending the command, you discard any noise that may have been received after the last command.
	pub fn discard_input_buffer(&self) -> std::io::Result<()> {
		self.inner.with_raw(|raw| raw.discard_input_buffer())
	}

	/// Discard the kernel output buffers for the serial port.
	///
	/// When you write to a serial port, the data is generally put in a buffer by the OS to be transmitted by the actual device later.
	/// This function clears that buffer: any untransmitted data is discarded by the OS.
	pub fn discard_output_buffer(&self) -> std::io::Result<()> {
		self.inner.with_raw(|raw| raw.discard_input_buffer())
	}

	/// Set the state of the Ready To Send line.
	///
	/// If hardware flow control is enabled on the serial port, it is platform specific what will happen.
	/// The function may fail with an error or it may silently be ignored.
	/// It may even succeed and interfere with the flow control.
	pub fn set_rts(&self, state: bool) -> std::io::Result<()> {
		self.inner.with_raw(|raw| raw.set_rts(state))
	}

	/// Read the state of the Clear To Send line.
	///
	/// If hardware flow control is enabled on the serial port, it is platform specific what will happen.
	/// The function may fail with an error, it may return a bogus value, or it may return the actual state of the CTS line.
	pub fn read_cts(&self) -> std::io::Result<bool> {
		self.inner.with_raw(|raw| raw.read_cts())
	}

	/// Set the state of the Data Terminal Ready line.
	///
	/// If hardware flow control is enabled on the serial port, it is platform specific what will happen.
	/// The function may fail with an error or it may silently be ignored.
	pub fn set_dtr(&self, state: bool) -> std::io::Result<()> {
		self.inner.with_raw(|raw| raw.set_dtr(state))
	}

	/// Read the state of the Data Set Ready line.
	///
	/// If hardware flow control is enabled on the serial port, it is platform specific what will happen.
	/// The function may fail with an error, it may return a bogus value, or it may return the actual state of the DSR line.
	pub fn read_dsr(&self) -> std::io::Result<bool> {
		self.inner.with_raw(|raw| raw.read_dsr())
	}

	/// Read the state of the Ring Indicator line.
	///
	/// This line is also sometimes also called the RNG or RING line.
	pub fn read_ri(&self) -> std::io::Result<bool> {
		self.inner.with_raw(|raw| raw.read_ri())
	}

	/// Read the state of the Carrier Detect (CD) line.
	///
	/// This line is also called the Data Carrier Detect (DCD) line
	/// or the Receive Line Signal Detect (RLSD) line.
	pub fn read_cd(&self) -> std::io::Result<bool> {
		self.inner.with_raw(|raw| raw.read_cd())
	}

	/// Get the RS-4xx mode of the serial port transceiver.
	///
	/// This is currently only supported on Linux.
	///
	/// Not all serial ports can be configured in a different mode by software.
	/// Some serial ports are always in RS-485 or RS-422 mode,
	/// and some may have hardware switches or jumpers to configure the transceiver.
	/// In those cases, this function will usually report an error or [`rs4xx::TransceiverMode::Default`],
	/// even though the serial port is configured is RS-485 or RS-422 mode.
	///
	/// Note that driver support for this feature is very limited and sometimes inconsistent.
	/// Please read all the warnings in the [`rs4xx`] module carefully.
	#[cfg(any(feature = "doc", all(feature = "rs4xx", target_os = "linux")))]
	#[cfg_attr(feature = "doc-cfg", doc(cfg(all(feature = "rs4xx", target_os = "linux"))))]
	pub fn get_rs4xx_mode(&self) -> std::io::Result<rs4xx::TransceiverMode> {
		self.inner.with_raw(|raw| raw.get_rs4xx_mode())
	}

	/// Set the RS-4xx mode of the serial port transceiver.
	///
	/// This is currently only supported on Linux.
	///
	/// Not all serial ports can be configured in a different mode by software.
	/// Some serial ports are always in RS-485 or RS-422 mode,
	/// and some may have hardware switches or jumpers to configure the transceiver.
	/// In that case, this function will usually return an error,
	/// but the port can still be in RS-485 or RS-422 mode.
	///
	/// Note that driver support for this feature is very limited and sometimes inconsistent.
	/// Please read all the warnings in the [`rs4xx`] module carefully.
	#[cfg(any(feature = "doc", all(feature = "rs4xx", target_os = "linux")))]
	#[cfg_attr(feature = "doc-cfg", doc(cfg(all(feature = "rs4xx", target_os = "linux"))))]
	pub fn set_rs4xx_mode(&self, mode: impl Into<rs4xx::TransceiverMode>) -> std::io::Result<()> {
		self.inner.with_raw(|raw| raw.set_rs4xx_mode(mode))
	}
}

impl AsyncRead for SerialPort {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
		buf: &mut tokio::io::ReadBuf<'_>,
	) -> Poll<std::io::Result<()>> {
		self.get_mut().inner.poll_read(cx, buf)
	}
}

impl AsyncWrite for SerialPort {
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
		buf: &[u8],
	) -> Poll<std::io::Result<usize>> {
		self.get_mut().inner.poll_write(cx, buf)
	}

	fn poll_write_vectored(
		self: Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
		bufs: &[IoSlice<'_>],
	) -> Poll<Result<usize, std::io::Error>> {
		self.get_mut().inner.poll_write_vectored(cx, bufs)
	}

	fn poll_flush(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), std::io::Error>> {
		// We can't do `tcdrain()` asynchronously :(
		Poll::Ready(Ok(()))
	}

	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<(), std::io::Error>> {
		self.get_mut().inner.poll_shutdown(cx)
	}
}
