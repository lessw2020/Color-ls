fn format_filename_with_indicators(file: &FileInfo, use_color: bool, show_counts: bool) -> String {
    let colored_name = colorize_filename(file, use_color);

    if file.is_dir && show_counts {
        match file.dir_count {
            Some(count) => {
                if use_color {
                    if let Some(color) = get_file_color(file) {
                        format!("{}{}{}{}", 
                            colored_name,
                            "(".color(color),
                            count.to_string().color(Color::BrightBlack),
                            ")".color(color)
                        )
                    } else {
                        format!("{colored_name}({count})")
                    }
                } else {
                    format!("{colored_name}({count})")
                }
            },
            None => {
                if use_color {
                    if let Some(color) = get_file_color(file) {
                        format!("{}{}{}{}", 
                            colored_name,
                            "[".color(color),
                            "?".color(Color::BrightBlack),
                            "]".color(color)
                        )
                    } else {
                        format!("{}[?]", colored_name)
                    }
                } else {
                    format!("{}[?]", colored_name)
                }
            }
        }
    } else if file.is_dir {
        // Only show "/" when counts are disabled
        if use_color {
            if let Some(color) = get_file_color(file) {
                format!("{}{}", colored_name, "/".color(color))
            } else {
                format!("{}/", colored_name)
            }
        } else {
            format!("{}/", colored_name)
        }
    } else {
        colored_name
    }
}
