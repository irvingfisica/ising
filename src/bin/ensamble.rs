use std::error::Error;
use ising::simulacion_sqgrids::construir_temps;
use ising::simulacion_sqgrids::simular_temperatura;
use ising::simulacion_sqgrids::{crear_carpeta_ejecucion,crear_sistema_base};

fn main() -> Result<(), Box<dyn Error>> {

    ensamble_grid()?;

    Ok(())
}

fn ensamble_grid() -> Result<(), Box<dyn Error>> {
    let temperaturas = construir_temps();

    let carpeta = crear_carpeta_ejecucion().map_err(|e| format!("{e}"))?;

    crear_sistema_base(&carpeta).map_err(|e| format!("{e}"))?;

    println!(
        "Resultados: {}",
        carpeta.display()
    );


    for temperatura in temperaturas {

        println!(
            "Simulando T = {:.6}",
            temperatura
        );

        simular_temperatura(temperatura, &carpeta).map_err(|e| format!("{e}"))?;
    };

    Ok(())
}