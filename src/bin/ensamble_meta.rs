use std::error::Error;
use ising::simulacion_sqgrids::construir_temps_propors;
use ising::simulacion_sqgrids::simular_temperatura_proporcion;
use ising::simulacion_sqgrids::{condiciones,crear_carpeta_ejecucion,crear_sistema_base};

fn main() -> Result<(), Box<dyn Error>> {

    ensamble_grid_meta()?;

    Ok(())
}

fn ensamble_grid_meta() -> Result<(), Box<dyn Error>> {
    let tuplas = construir_temps_propors();

    let carpeta = crear_carpeta_ejecucion().map_err(|e| format!("{e}"))?;

    crear_sistema_base(&carpeta).map_err(|e| format!("{e}"))?;

    println!(
        "Resultados: {}",
        carpeta.display()
    );

    condiciones(tuplas.len());

    for (temperatura,proporcion) in tuplas {
        println!(
            "Simulando T = {:.6}, P = {:.2}",
            temperatura,
            proporcion
        );

        simular_temperatura_proporcion(temperatura, proporcion, &carpeta).map_err(|e| format!("{e}"))?;
    }

    Ok(())
}
