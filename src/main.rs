mod charset_gen;

extern crate num;
extern crate rand;
#[macro_use]
extern crate lazy_static;
use crate::charset_gen::{ascii_chars, hiragana_chars};
use crate::rand::prelude::SliceRandom;
use crate::rand::{thread_rng, Rng};
use std::fmt::{Display, Formatter};
use std::rc::Rc;

enum Direction {
    Up,
    Down,
    Left,
    Right,
}
enum AnsiCommand {
    RelMove(u32, Direction),
    EraseLine,
    Move(u32, u32),
    Color256(u8, bool),
    Cell(char),
    Reset,
}
#[derive(Copy, Clone)]
enum ColorPickerOption {
    Greens,
    Any,
}
struct Screen {
    columns: usize,
    rows: usize,
    cells: Vec<(char, u8)>, //Character,color
    columns_producing: Vec<i8>,
    color_picker: ColorPickerOption,
    character_sets: Vec<Vec<char>>,
}
struct ScreenIterator<'a> {
    screen: &'a Screen,
    column: usize,
    row: usize,
    phase: u32, // 0 for start, 1 for color, 2 for color
}
impl<'a> Iterator for ScreenIterator<'a> {
    type Item = AnsiCommand;
    fn next(&mut self) -> Option<Self::Item> {
        use AnsiCommand::*;
        if self.row == self.screen.rows - 1 && self.screen.columns == self.column {
            None
        } else if self.column == self.screen.columns {
            self.row += 1;
            self.column = 0;
            self.phase = 0;
            Some(Move(0, self.row as u32))
        } else if self.row == 0 && self.column == 0 && self.phase == 0 {
            self.phase += 1;
            Some(Move(0, 0))
        } else if self.phase == 1 {
            self.phase += 1;
            Some(Color256(
                self.screen.get_cell(self.column, self.row).1,
                false,
            ))
        } else {
            let (c, _) = self.screen.get_cell(self.column, self.row);
            self.phase = 0;
            self.column += 1;
            Some(Cell(c))
        }
    }
}
trait ColorPicker256 {
    fn get_color() -> u8;
}
struct GreenPicker;
struct RainbowPicker;
impl ColorPicker256 for RainbowPicker {
    fn get_color() -> u8 {
        thread_rng().gen_range(0u8, 255u8)
    }
}
impl ColorPicker256 for GreenPicker {
    fn get_color() -> u8 {
        let mut rng = rand::thread_rng();
        *[46u8, 47u8, 48u8, 34u8, 70u8, 64u8, 255u8]
            .choose(&mut rng)
            .unwrap()
    }
}

impl Screen {
    fn new(color_pick: ColorPickerOption, charsets: CharsetChoices) -> Screen {

        let mut ret: Screen = Screen {
            columns: 80,
            rows: 24,
            cells: Vec::new(),
            columns_producing: Vec::new(),
            color_picker: color_pick,
            character_sets:charsets.to_char_vecs()
        };
        use terminal_size::{terminal_size, Height, Width};

        let size = terminal_size();
        if let Some((Width(w), Height(h))) = size {
            ret.columns = w as usize;
            ret.rows = h as usize;
        } else {
            ret.columns = 80;
            ret.rows = 24;
        }
        if ret.rows > 1 {
            ret.rows -= 1;
        }
        for _j in 0..ret.columns {
            ret.columns_producing.push(0);
        }
        for _i in 0..ret.rows {
            for _j in 0..ret.columns {
                ret.cells.push((' ', 255));
            }
        }
        ret
    }
    fn get_cell(&self, x: usize, y: usize) -> (char, u8) {
        self.cells[x + y * self.columns]
    }
    fn set_cell(&mut self, x: usize, y: usize, c: Option<char>, color: Option<u8>) {
        let c = match c {
            Some(x) => x,
            None => self.get_cell(x, y).0,
        };
        let q = match color {
            Some(x) => x,
            None => self.get_cell(x, y).1,
        };
        self.cells[x + self.columns * y] = (c, q);
    }
    fn produce(&mut self) {
        let mut rng = rand::thread_rng();
        let mut skip_next = 0;
        for x in 0..self.columns {
            if skip_next > 0 {
                self.set_cell(x, 0, None, None);
                skip_next -= 1;
                continue;
            }
            let i = self.columns_producing[x];
            if i > 0 {
                let c = if rng.gen() {
                    Some(self.dispatch())
                } else {
                    None
                };
                let current_char = *self
                    .character_sets
                    .choose(&mut rng)
                    .unwrap()
                    .choose(&mut rng)
                    .unwrap();
                if let Some(x) = unicode_width::UnicodeWidthChar::width(current_char) {
                    skip_next = if x > 1 { x - 1 } else { 0 };
                }
                self.set_cell(x, 0, Some(current_char), c);
            } else {
                self.set_cell(x, 0, Some(' '), None);
            }
            self.columns_producing[x] -= 1;
        }
    }
    fn dispatch(&self) -> u8 {
        use ColorPickerOption::*;
        match self.color_picker {
            Greens => GreenPicker::get_color(),
            Any => RainbowPicker::get_color(),
        }
    }
    fn reset_producing(&mut self) {
        for i in 0..self.columns {
            if self.columns_producing[i] < -((self.rows / 2) as i8) && rand::random() {
                self.columns_producing[i] = thread_rng().gen_range(5i8, self.rows as i8);
            }
        }
    }
    fn tick(&mut self) {
        for y in (1..self.rows).rev() {
            for x in 0..self.columns {
                let (character, color) = self.get_cell(x, y - 1);
                let color = if thread_rng().gen_range(0, 15) > 13 {
                    Some(color)
                } else {
                    None
                };
                self.set_cell(x, y, Some(character), color);
            }
        }
        self.produce();
        self.reset_producing();
    }
    fn to_commands(&self, result: &mut Vec<AnsiCommand>) {
        use AnsiCommand::*;
        result.push(Move(0, 0));
        for y in 0..self.rows {
            result.push(Move(1, y as u32));
            let mut effective_x = 0u32;
            for x in 0..self.columns {
                let (character, color) = self.get_cell(x, y);
                // Uh, I guess this works? It seems kinda weird.
                effective_x += unicode_width::UnicodeWidthChar::width(character).unwrap() as u32;
                result.push(Move(effective_x, y as u32));
                result.push(Color256(color, false));
                result.push(Cell(character));
            }
        }
    }
}
#[derive(Default)]
struct CharsetChoices {
    ascii: bool,
    hiragana: bool,
    cjk: bool,
}
impl CharsetChoices {
    fn read_from_arg(&mut self, arg: &str) {
        let to_set = !arg.starts_with("-");
        if arg.ends_with("ascii") {
            self.ascii = to_set;
        } else if arg.ends_with("hiragana") {
            self.hiragana = to_set;
        } else if arg.ends_with("cjk") {
            self.cjk = to_set;
        }
    }
    fn to_char_vecs(&self) -> Vec<Vec<char>> {
        let mut ret = Vec::new();
        if self.ascii {
            ret.push(charset_gen::ascii_chars());
        }
        if self.hiragana {
            ret.push(charset_gen::hiragana_chars());
        }
        if self.cjk {
            ret.push(charset_gen::cjk_chars());
        }
        ret
    }
}
fn print_help() {
    println!("Usage: {}", std::env::args().nth(0).unwrap());
    println!("-any\tUse any of the 256 colors available");
    println!("-charset <set>\t\tEnable a charset");
    println!("\t\tascii Enable ascii characters");
    println!("\t\tcjk Enable the CJK charset");
    println!("\t\thiragana Enable the hiragana set");
    println!("\t\tPrefixing the charset with a '-' will disable that charset. Useful for ascii");
    println!("-frame <N>\tPrint N frames a second");
}
fn main() {
    use std::io::Write;
    let mut color_picker = ColorPickerOption::Greens;
    let mut state = 0;
    let mut milliseconds_per_frame = 1000 / 30;
    // TODO add a way to choose which character ranges are enabled.
    let mut charset_choices = CharsetChoices {
        ascii: true,
        ..Default::default()
    };
    for arg in std::env::args() {
        if state == 1 {
            milliseconds_per_frame = arg.parse().unwrap();
            milliseconds_per_frame = 1000 / milliseconds_per_frame;
            state = 0;
        }
        if state == 2 {
            charset_choices.read_from_arg(&arg);
            state = 0;
        }
        if arg == "-any" {
            color_picker = ColorPickerOption::Any;
        } else if arg == "-frame" {
            state = 1;
        } else if arg == "-charset" {
            state = 2;
        }
        if arg == "-help" || arg == "-h" {
            print_help();
            return;
        }
    }
    let mut screen = Screen::new(color_picker, charset_choices);
    let mut commands: Vec<AnsiCommand> = Vec::new();
    let mut output_buffer: String = String::new();
    loop {
        commands.clear();
        screen.to_commands(&mut commands);
        output_buffer.clear();
        for i in commands.iter() {
            output_buffer.push_str(&i.to_string());
        }
        print!("{}", output_buffer);
        std::io::stdout().flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(milliseconds_per_frame));
        screen.tick();
    }
}
impl Display for AnsiCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use AnsiCommand::*;
        match self {
            EraseLine => write!(f, "\x1b[0K"),
            Move(x, y) => write!(f, "\x1b[{};{}H", y + 1, x + 1),
            AnsiCommand::Color256(color, background) => {
                write!(f, "\x1b[{};5;{}m", if *background { 48 } else { 38 }, color)
            }
            AnsiCommand::RelMove(amount, dir) => write!(
                f,
                "\x1b[{}{}",
                amount,
                match dir {
                    Direction::Up => 'A',
                    Direction::Down => 'B',
                    Direction::Left => 'C',
                    Direction::Right => 'D',
                }
            ),
            Cell(x) => write!(f, "{}", x),
            Reset => write!(f, "\x1b[0m"),
        }
    }
}
