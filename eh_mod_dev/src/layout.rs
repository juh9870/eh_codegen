use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct Layout {
    pub layout: Vec<char>,
    pub size: usize,
}

impl Layout {
    pub const S1: &'static str = "1";
    pub const S2: &'static str = "1111";
    pub const S3: &'static str = "111111111";
    pub const S4: &'static str = "1111111111111111";
    pub const S5: &'static str = "1111111111111111111111111";

    pub fn new(size: usize) -> Self {
        Self {
            layout: vec!['0'; size * size],
            size,
        }
    }
    pub fn new_square(size: usize, brush: char) -> Self {
        Self {
            layout: vec![brush; size * size],
            size,
        }
    }
    pub fn new_rect(width: usize, height: usize, brush: char) -> Self {
        let side = width.max(height);
        let mut layout = Self::new(side);

        layout.rect(0, 0, width, height, brush);

        layout
    }

    pub fn rect(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        brush: char,
    ) -> &mut Layout {
        let w = self.size;
        for y in y..(y + height) {
            let oy = y * w;
            for x in x..(x + width) {
                self.layout[x + oy] = brush;
            }
        }
        self
    }
}

impl Display for Layout {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.layout.iter().collect::<String>())
    }
}

impl From<Layout> for String {
    fn from(val: Layout) -> Self {
        val.layout.into_iter().collect::<String>()
    }
}
