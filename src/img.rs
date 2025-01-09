use crate::InputData;

pub fn image<const RESPECT_HOLES: bool>(input_data: InputData) {
    for (i, map) in input_data.maps.iter().enumerate() {
        let img = map.image(RESPECT_HOLES, 5, 5);
        img.save(format!("map_{i}.png")).unwrap();
    }
}
