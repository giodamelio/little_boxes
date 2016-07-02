pub struct Charset {
    pub horizontal: char,
    pub vertical: char,
    pub corner_up_left: char,
    pub corner_up_right: char,
    pub corner_down_left: char,
    pub corner_down_right: char,
    pub t_right: char,
    pub t_left: char,
}

pub fn get_charset(input: &str) -> Option<Charset> {
    match input {
        "thick" => {
            Some(Charset {
                horizontal: '━',
                vertical: '┃',
                corner_up_left: '┏',
                corner_up_right: '┓',
                corner_down_left: '┗',
                corner_down_right: '┛',
                t_right: '┣',
                t_left: '┫',
            })
        }
        "thin" => {
            Some(Charset {
                horizontal: '─',
                vertical: '│',
                corner_up_left: '┌',
                corner_up_right: '┐',
                corner_down_left: '└',
                corner_down_right: '┘',
                t_right: '├',
                t_left: '┤',
            })
        }
        "double" => {
            Some(Charset {
                horizontal: '═',
                vertical: '║',
                corner_up_left: '╔',
                corner_up_right: '╗',
                corner_down_left: '╚',
                corner_down_right: '╝',
                t_right: '╠',
                t_left: '╣',
            })
        }
        "box" => {
            Some(Charset {
                horizontal: '█',
                vertical: '█',
                corner_up_left: '█',
                corner_up_right: '█',
                corner_down_left: '█',
                corner_down_right: '█',
                t_right: '█',
                t_left: '█',
            })
        }
        "rounded" => {
            Some(Charset {
                horizontal: '─',
                vertical: '│',
                corner_up_left: '╭',
                corner_up_right: '╮',
                corner_down_left: '╰',
                corner_down_right: '╯',
                t_right: '├',
                t_left: '┤',
            })
        }
        "dot" => {
            Some(Charset {
                horizontal: '⠶',
                vertical: '⣿',
                corner_up_left: '⣶',
                corner_up_right: '⣶',
                corner_down_left: '⠿',
                corner_down_right: '⠿',
                t_right: '⡷',
                t_left: '⢾',
            })
        }
        _ => None,
    }
}
