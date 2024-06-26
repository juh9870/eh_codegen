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

    pub const V1X2: &'static str = "0101";
    pub const V1X3: &'static str = "010010010";
    pub const V1X4: &'static str = "0010001000100010";
    pub const V1X5: &'static str = "0010000100001000010000100";

    pub const V2X3: &'static str = "011011011";
    pub const V2X4: &'static str = "0110011001100110";
    pub const V2X5: &'static str = "0011000110001100011000110";

    pub const V3X4: &'static str = "0111011101110111";
    pub const V3X5: &'static str = "0111001110011100111001110";

    pub const H1X2: &'static str = "0011";
    pub const H1X3: &'static str = "000111000";

    pub const H2X3: &'static str = "000111111";

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
