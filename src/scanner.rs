pub struct Scanner<R> {
    reader: R,
    pub buffer: String,
    read: usize,
}

impl<R> Scanner<R>
where
    R: std::io::BufRead,
{
    pub fn new(read: R) -> Self {
        Self {
            reader: read,
            buffer: String::new(),
            read: 0,
        }
    }

    #[allow(dead_code)]
    pub fn read_trim(&mut self) {
        self.buffer.clear();
        self.reader.read_line(&mut self.buffer).unwrap();
        self.read = 0;
        let new_len = self.buffer.trim_end().len();
        unsafe {
            self.buffer.as_mut_vec().set_len(new_len);
        }
    }

    #[allow(dead_code)]
    pub fn has_line_ended(&mut self) -> bool {
        self.read >= self.buffer.len()
    }

    #[allow(dead_code)]
    pub fn has_ended(&mut self) -> bool {
        if self.read >= self.buffer.len() {
            self.read_trim();
        }
        self.buffer.is_empty()
    }

    #[allow(dead_code)]
    pub fn read<T: std::str::FromStr>(&mut self) -> T
    where
        T::Err: std::fmt::Debug,
    {
        if self.read >= self.buffer.len() {
            self.read_trim();
        }

        let new_read = self.buffer[self.read..]
            .find(|c: char| c.is_whitespace())
            .map(|v| v + self.read)
            .unwrap_or(self.buffer.len());

        let result = unsafe { self.buffer.get_unchecked(self.read..new_read) };
        self.read = new_read + 1;
        if result.is_empty() {
            self.read()
        } else {
            result.parse().unwrap()
        }
    }

    #[allow(dead_code)]
    pub fn read_line(&mut self) -> &str {
        self.read_trim();
        self.read = self.buffer.len();
        &self.buffer
    }
}