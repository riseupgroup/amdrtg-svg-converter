use {
    std::collections::HashMap,
    svg::{node::element::{path::{Command, Data, Parameters, Position}, Path, Rectangle}, parser::Event, Document, Node},
};

fn main() {
    let mut content = String::new();
    let mut paths: HashMap<char, (f32, String)> = HashMap::new();
    let mut chars = Vec::new();
    for event in svg::open("font.svg", &mut content).unwrap() {
        if let Event::Tag("glyph", _, attributes) = event {
            let name: String = attributes.get("unicode").unwrap().clone().into();
            let horiz_adv_x: f32 = attributes.get("horiz-adv-x").unwrap().parse().unwrap();
            let path = attributes.get("d").unwrap();
            let mut char_string = String::new();
            let char = html_escape::decode_html_entities_to_string(name, &mut char_string).chars().next().unwrap();
            
            if char.is_ascii() {
                if !chars.contains(&char.to_ascii_lowercase()) {
                    chars.push(char.to_ascii_lowercase());
                }
                paths.insert(char, (horiz_adv_x, path.clone().into()));
            }
        }
    }

    let mut rect = None;
    for char in 'A'..='Z' {
        if let Some((_, path)) = paths.get(&char) {
            let mut commands = Data::parse(path).unwrap().into();
            rect = Some(get_rect(&mut commands));
        }
    }
    let rect = rect.unwrap();

    for char in chars {
        let char_upper = char.to_ascii_uppercase();
        if let Some((width, path_upper)) = paths.get(&char_upper) {
            let path_lower = if char != char_upper {
                match paths.get(&char) {
                    Some(x) => Some((x.0, x.1.as_str())),
                    None => None,
                }
            } else {
                None
            };
            process_glyph(char, rect, path_lower, (*width, path_upper));
        }
    }
}

fn process_glyph(char: char, rect: Rect, path_lower: Option<(f32, &str)>, path_upper: (f32, &str)) {
    let mut commands_upper: Vec<Command> = Data::parse(path_upper.1).unwrap().into();

    flip_and_align_path(&mut commands_upper, rect, path_upper.0);
    save(char.to_ascii_uppercase(), commands_upper, rect, true);

    if let Some((width, path_lower)) = path_lower {
        let mut commands_lower: Vec<Command> = Data::parse(path_lower).unwrap().into();
        flip_and_align_path(&mut commands_lower, rect, width);
        save(char, commands_lower, rect, false);
    }
}

fn get_rect(commands: &mut Vec<Command>) -> Rect {
    let mut rect = Rect::new();
    let mut pos = None;

    for_each_command(commands, |position, _params, points, point_pair_size| {
        for i in (0..points.len()).skip(point_pair_size-1).step_by(point_pair_size) {
            let point = points[i];
            let point_abs = match position {
                Position::Absolute => {
                    pos = Some(point);
                    point
                }
                Position::Relative => {
                    if let Some(pos) = &mut pos {
                        pos.0 += point.0;
                        pos.1 += point.1;
                        *pos
                    } else {
                        panic!()
                    }
                }
            };
            rect.extend(point_abs);
        }
    });
    rect
}

fn flip_and_align_path(commands: &mut Vec<Command>, rect: Rect, width: f32) {
    let offset = ((rect.width() - width) / 2.0) + rect.min.0;
    for_each_command(commands, |position, params, points, _| {
        let mut params_new = Vec::with_capacity(points.len() * 2);
        for point in points {
            match position {
                Position::Absolute => {
                    params_new.push(offset + point.0 - rect.min.0);
                    params_new.push(rect.max.1 - point.1);
                }
                Position::Relative => {
                    params_new.push(point.0);
                    params_new.push(-point.1);
                }
            }
        }
        *params = params_new.into();
    });
}

fn save(char: char, commands: Vec<Command>, rect: Rect, background: bool) {
    let path = Path::new().set("d", Data::from(commands));
    let mut document = Document::new().set("viewBox", (0.0, 0.0, rect.width(), rect.height()));
    if background {
        let background = Rectangle::new().set("fill", "white").set("x", 0.0).set("y", 0.0).set("width", rect.width()).set("height", rect.height());
        Node::append(&mut document, background);
    }
    let file_path = match char {
        '/' => String::from("./out/slash.svg"),
        x => format!("./out/{x}.svg")
    };
    svg::save(file_path, &document.add(path)).unwrap();
}

fn for_each_command(commands: &mut Vec<Command>, mut callback: impl FnMut(&Position, &mut Parameters, Vec<(f32, f32)>, usize)) {
    for command in commands.iter_mut() {
        let point_pair_size = match &command {
            Command::QuadraticCurve(_, _) => 2,
            Command::CubicCurve(_, _) => 3,
            Command::SmoothCubicCurve(_, _) => 2,
            _ => 1,
        };
        match command {
            Command::Move(position, params)
            | Command::Line(position, params)
            //| Command::HorizontalLine(position, params)
            //| Command::VerticalLine(position, params)
            | Command::QuadraticCurve(position, params)
            | Command::SmoothQuadraticCurve(position, params)
            | Command::CubicCurve(position, params)
            | Command::SmoothCubicCurve(position, params)
            //| Command::EllipticalArc(position, params)
            => {
                let mut points = Vec::with_capacity(params.len()/2);
                let mut iter = params.iter();
                while let Some(x) = iter.next() {
                    if let Some(y) = iter.next() {
                        points.push((*x, *y));
                    }
                }
                callback(position, params, points, point_pair_size)
            }
            Command::Close
            | Command::HorizontalLine(_, _)
            | Command::VerticalLine(_, _)
            | Command::EllipticalArc(_, _) => (),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    min: (f32, f32),
    max: (f32, f32),
}

impl Rect {
    pub fn new() -> Self {
        Self {
            min: (f32::INFINITY, f32::INFINITY),
            max: (f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }

    pub fn extend(&mut self, point: (f32, f32)) {
        if point.0 < self.min.0 {
            self.min.0 = point.0;
        }
        if point.0 > self.max.0 {
            self.max.0 = point.0;
        }

        if point.1 < self.min.1 {
            self.min.1 = point.1;
        }
        if point.1 > self.max.1 {
            self.max.1 = point.1;
        }
    }

    pub fn width(&self) -> f32 {
        self.max.0 - self.min.0
    }

    pub fn height(&self) -> f32 {
        self.max.1 - self.min.1
    }
}
