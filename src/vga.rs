use core::fmt;
use lazy_static::lazy_static;
use spin::{Mutex, Once};

const VGA_HEIGHT: usize = 480;
const VGA_WIDTH: usize = 640;
const LINE_SIZE: usize = 16;

lazy_static! {
    pub static ref WRITER: Once<Mutex<Writer>> = Once::new();
}

pub fn init_vga() {
    let vga_offset = 0x_7F55_AAAA_0000u64;
    let vga_base = vga_offset as *mut u32;
    for y in 0..480 {
        for x in 0..640 {
            unsafe {
                vga_base.add(x + y * 640).write(0);
            }
        }
    }
    WRITER.call_once(|| {
        let vga_base = 0x_7F55_AAAA_0000u64 as *mut u32;
        Mutex::new(Writer {
            column_position: 0,
            buffer: unsafe { core::slice::from_raw_parts_mut(vga_base, VGA_WIDTH * VGA_HEIGHT) },
        })
    });
}

const BUFFER_HEIGHT: usize = VGA_HEIGHT / LINE_SIZE;
const BUFFER_WIDTH: usize = VGA_WIDTH / LINE_SIZE;

pub struct Writer {
    column_position: usize,
    buffer: &'static mut [u32],
}

impl Writer {
    fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7E | b'\n' => self.write_byte(byte),
                _ => self.write_byte(0xfe),
            }
        }
    }

    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                for y in 16 * row..16 * (row + 1) {
                    for x in 16 * col..16 * (col + 1) {
                        let bm_y = y % 16 / 2;
                        let bm_x = x % 16 / 2;
                        let bm = font8x8::legacy::BASIC_LEGACY[byte as usize];
                        const W: u32 = 0xFFFFFF;
                        self.buffer[x + y * VGA_WIDTH] = (bm[bm_y] & (1 << bm_x)) as u32 * W;
                    }
                }
                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {
        for i in 0..VGA_WIDTH * (VGA_HEIGHT - LINE_SIZE) {
            self.buffer[i as usize] = self.buffer[i as usize + VGA_WIDTH * LINE_SIZE as usize];
        }

        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        for col in VGA_WIDTH * (LINE_SIZE * row)..VGA_WIDTH * (LINE_SIZE * (row + 1)) {
            self.buffer[col] = 0;
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.get().unwrap().lock().write_fmt(args).unwrap();
    });
}

#[cfg(test)]
mod tests {
    #[test_case]
    fn println_simple() {
        println!("simple println test");
    }

    #[test_case]
    fn println_many() {
        for _ in 0..5 {
            println!("a line");
        }
    }
}
