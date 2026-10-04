use std::env;
use std::error::Error;

use ising::graficas;

// Gráficas estáticas (SVG) de un directorio de salida:
//     cargo run --example graficas --features graficas -- ruta/a/datos [ruta/destino]
fn main() -> Result<(), Box<dyn Error>> {
    let datos = env::args().nth(1).unwrap_or_else(|| "datos".to_string());
    let destino = env::args().nth(2).unwrap_or_else(|| format!("{}/graficas", datos));

    for ruta in graficas::graficar(&datos, &destino)? {
        println!("{}", ruta.display());
    }

    Ok(())
}
