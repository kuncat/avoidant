fn main() {
    let mut maps = serde_json::Map::new();
    for kind in [
        "flat",
        "icosahedron",
        "spheroid",
        "geodesicIcosphere",
        "tetrahedron",
        "cube",
        "octahedron",
        "dodecahedron",
    ] {
        for (name, size) in [("small", 80), ("medium", 160), ("large", 320)] {
            let shape = match kind {
                "spheroid" => {
                    serde_json::json!({"kind":kind,"radiusX":50,"radiusY":40,"radiusZ":50})
                }
                "geodesicIcosphere" => {
                    serde_json::json!({"kind":kind,"radius":50,"subdivisions":1})
                }
                _ => serde_json::json!({"kind":kind,"radius":50}),
            };
            let options = serde_json::json!({"numCells":size,"rngSeed":42,"shape":shape,"spikiness":0.8,"elevationMin":0,"elevationMax":6});
            let key = if kind == "icosahedron" {
                name.to_string()
            } else {
                format!("{kind}-{name}")
            };
            let neighbors = avoidant::competitive_catalog_adjacency(
                serde_json::from_value(options.clone()).unwrap(),
            )
            .unwrap();
            maps.insert(
                key,
                serde_json::json!({"options":options,"neighbors":neighbors}),
            );
        }
    }
    println!("{}", serde_json::to_string(&maps).unwrap());
}
