use std::env;
use std::error::Error;

use ising::visor;

// Instala el visor web en un directorio de salida ya existente:
//     cargo run --example instalar_visor -- ruta/a/datos
fn main() -> Result<(), Box<dyn Error>> {
    let dir = env::args().nth(1).unwrap_or_else(|| "datos".to_string());
    let ruta = visor::instalar(&dir)?;
    println!("Visor: {}", ruta.display());
    Ok(())
}
