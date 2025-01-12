use std::io;

use crate::{Direction, InputData};

pub fn graph<const RESPECT_HOLES: bool>(input_data: InputData) {
    let mut file = std::fs::File::create("graph.dot").unwrap();
    gen_graph::<_, RESPECT_HOLES>(input_data, &mut file).unwrap();
}

pub fn gen_graph<W: io::Write, const RESPECT_HOLES: bool>(
    input_data: InputData,
    mut write: W,
) -> io::Result<()> {
    let InputData {
        width,
        height,
        maps,
    } = input_data;

    write!(write, "DiGraph G {{")?;

    for x1 in 0..width {
        for y1 in 0..height {
            for x2 in 0..width {
                for y2 in 0..height {
                    let positions = [[x1, y1], [x2, y2]];

                    for dir in Direction::ALL {
                        let blocked: [_; 2] =
                            std::array::from_fn(|i| dir.blocked(positions[i], &maps[0]));

                        if blocked.into_iter().any(|v| !v) {
                            let new_positions: [_; 2] = std::array::from_fn(|i| {
                                if blocked[i] {
                                    positions[i]
                                } else {
                                    dir.apply(positions[i])
                                }
                            });

                            write!(write, r#""{:?}" -> "{:?}""#, positions, new_positions)?;
                        }
                    }
                }
            }
        }
    }

    write!(
        write,
        r#""{:?}" [color=green];"{:?}" [color=red];"#,
        [[0; 2]; 2],
        [[width - 1, height - 1]; 2]
    )?;
    write!(write, "}}")?;

    Ok(())
}
