# serial2-tokio

Serial port communication for [`tokio`] using [`serial2`].

The `serial2-tokio` crate provides a cross-platform interface to serial ports.
It aims to provide a simpler interface than other alternatives.

Currently supported features:
* Simple interface: one [`SerialPort`] struct for all supported platforms.
* List available ports.
* Custom baud rates on all supported platforms except Solaris and Illumos.
* Concurrent reads and writes from multiple tasks, even on Windows.
* Purge the OS buffers (useful to discard read noise when the line should have been silent, for example).
* Read and control individual modem status lines to use them as general purpose I/O.
* Cross platform configuration of serial port settings:
  * Baud rate
  * Character size
  * Stop bits
  * Parity checks
  * Flow control
  * Read/write timeouts

You can open and configure a serial port in one go with [`SerialPort::open()`].
The second argument to `open()` must be a type that implements [`IntoSettings`].
In the simplest case, it is enough to pass a `u32` for the baud rate.
Doing that will also configure a character size of 8 bits with 1 stop bit and disables parity checks and flow control.
For full control over the applied settings, pass a closure that receives the the current [`Settings`] and return the desired settings.
If you do, you will almost always want to call [`Settings::set_raw()`] before changing any other settings.

The [`SerialPort`] struct implements the standard [`tokio::io::AsyncRead`] and [`tokio::io::AsyncWrite`] traits,
as well as [`read()`][`SerialPort::read()`] and [`write()`][`SerialPort::write()`] functions that take `&self` instead of `&mut self`.
This allows you to use the serial port concurrently from multiple tasks.

The [`SerialPort::available_ports()`] function can be used to get a list of available serial ports on supported platforms.

## Example
This example opens a serial port and echoes back everything that is read.

```rust
use serial2_tokio::SerialPort;

// On Windows, use something like "COM1" or "COM15".
let port = SerialPort::open("/dev/ttyUSB0", 115200)?;
let mut buffer = [0; 256];
loop {
    let read = port.read(&mut buffer).await?;
    port.write(&buffer[..read]).await?;
}
```

[`tokio`]: https://docs.rs/tokio/
[`serial2`]: https://docs.rs/serial2/
[`SerialPort`]: https://docs.rs/serial2-tokio/latest/serial2_tokio/struct.SerialPort.html
[`SerialPort::open()`]: https://docs.rs/serial2-tokio/latest/serial2_tokio/struct.SerialPort.html#method.open
[`IntoSettings`]: https://docs.rs/serial2-tokio/latest/serial2_tokio/trait.IntoSettings.html
[`Settings`]: https://docs.rs/serial2-tokio/latest/serial2_tokio/struct.Settings.html
[`Settings::set_raw()`]: https://docs.rs/serial2-tokio/latest/serial2_tokio/struct.Settings.html#method.set_raw
[`tokio::io::AsyncRead`]: https://docs.rs/tokio/latest/tokio/io/trait.AsyncRead.html
[`tokio::io::AsyncWrite`]: https://docs.rs/tokio/latest/tokio/io/trait.AsyncWrite.html
[`std::io::Read`]: https://doc.rust-lang.org/stable/std/io/trait.Read.html
[`std::io::Write`]: https://doc.rust-lang.org/stable/std/io/trait.Write.html
[`SerialPort::read()`]: https://docs.rs/serial2-tokio/latest/serial2_tokio/struct.SerialPort.html#method.read
[`SerialPort::write()`]: https://docs.rs/serial2-tokio/latest/serial2_tokio/struct.SerialPort.html#method.write
[`SerialPort::available_ports()`]: https://docs.rs/serial2-tokio/latest/serial2_tokio/struct.SerialPort.html#method.available_ports
