use std::error::Error;
use ising::simulacion_sqgrids:: simular_instancia;
use ising::simulacion_sqgrids::{crear_carpeta_ejecucion,crear_sistema_base};
use ising::sistema:: Inicial;

fn main() -> Result<(), Box<dyn Error>> {

    simular_grid(2.69, Inicial::Random)?;

    Ok(())
}

fn simular_grid(temperatura: f64, inicial: Inicial) -> Result<(), Box<dyn Error>> {

    let replica = 0;

    let carpeta = crear_carpeta_ejecucion().map_err(|e| format!("{e}"))?;

    crear_sistema_base(&carpeta).map_err(|e| format!("{e}"))?;

    println!(
        "Resultados: {}",
        carpeta.display()
    );

    println!(
            "Simulando T = {:.6}",
            temperatura
        );

    simular_instancia(temperatura, inicial, replica, &carpeta).map_err(|e| format!("{e}"))?;


    Ok(())
}
