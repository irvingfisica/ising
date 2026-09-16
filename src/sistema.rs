use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::writeln;
use rand::{Rng, RngExt};
use rand::distr::{Distribution,StandardUniform};
use std::fmt::Write as FmtWrite;
use std::io::{self, Write, BufRead};

pub struct Sistema {
    mapa: HashMap<String,usize>,
    elementos: Vec<Celda>,
    j: f64,
    h: f64,
    temp: f64,
}

impl Sistema {

    pub fn from_edges<R: BufRead, G: Rng>(archivo: R, j: f64, h: f64, temp: f64, inicial: Inicial, rng: &mut G) -> Result<Self, Box<dyn Error>> {

        let mut sistema = Sistema {
            mapa: HashMap::new(),
            elementos: Vec::new(),
            j,h,temp,
        };

        let mut conexiones: Vec<(String, String)> = Vec::new();

        for linea in archivo.lines() {

            let linea = linea?;

            let linea = linea.trim();

            if linea.is_empty() {
                continue;
            }

            let partes: Vec<&str> = linea.split(',').collect();

            if partes.len() != 2 {
                return Err(format!("Conexiones mal definidas: '{}'",linea).into());
            }

            let id_i = partes[0].trim().to_string();
            let id_j = partes[1].trim().to_string();

            conexiones.push((id_i,id_j));
        }

        for (id_i, id_j) in &conexiones {
            for id in [id_i, id_j] {
                if !sistema.mapa.contains_key(id) {

                    let estado = match inicial {
                        Inicial::Random => rng.random(),
                        Inicial::Negativo => Estado::Negativo,
                        Inicial::Positivo => Estado::Positivo,
                        Inicial::Parcial(prop) => {
                            if rng.random::<f32>() < prop {
                                Estado::Positivo
                            } else {
                                Estado::Negativo
                            }
                        }
                    };

                    let pos = sistema.elementos.len();

                    sistema.mapa.insert(id.clone(), pos);
                    sistema.elementos.push(Celda::new(id, estado))
                }
            }
        }

        for (id_i, id_j) in conexiones {
            let pos = *sistema.mapa.get(&id_i).ok_or("Nodo inexistente")?;
            let celda = &mut sistema.elementos[pos];

            celda.add_vecino(id_j);
        }

        for ele in sistema.elementos.iter_mut() {
            ele.seguir_vecinos(&sistema.mapa);
        }

        Ok(sistema)
    }

    pub fn square_grid<R: Rng>(n: usize, j: f64, h: f64, temp: f64, inicial: Inicial, rnd: &mut R) -> Self {
        let mut sistema = Sistema {
            mapa: HashMap::new(),
            elementos: Vec::new(),
            j,h,temp,
        };

        let mut cta = 0;
        for i_idx in 0..n {
            for j_idx in 0..n {
                let id:String = format!("{i_idx}-{j_idx}");
                let estado : Estado = match inicial {
                    Inicial::Random => rnd.random(),
                    Inicial::Negativo => Estado::Negativo,
                    Inicial::Positivo => Estado::Positivo,
                    Inicial::Parcial(prop) => {
                        if prop > (j_idx as f32 / n as f32) {
                            Estado::Positivo
                        } else {
                            Estado::Negativo
                        }
                    }
                };
                let mut celda = Celda::new(&id, estado);

                let up = if j_idx == 0 {
                    n - 1
                } else {
                    j_idx - 1
                };
                let down = if j_idx == n - 1 {
                    0
                } else {
                    j_idx + 1
                };
                let left = if i_idx == 0 {
                    n - 1
                } else {
                    i_idx - 1
                };
                let right = if i_idx == n - 1 {
                    0
                } else {
                    i_idx + 1
                };

                let v1 = format!("{i_idx}-{up}");
                let v2 = format!("{i_idx}-{down}");
                let v3 = format!("{left}-{j_idx}");
                let v4 = format!("{right}-{j_idx}");
                celda.add_vecino(v1);
                celda.add_vecino(v2);
                celda.add_vecino(v3);
                celda.add_vecino(v4);

                sistema.mapa.insert(id, cta);
                sistema.elementos.push(celda);
                cta = cta + 1;
            }
        }

        for ele in sistema.elementos.iter_mut() {
            ele.seguir_vecinos(&sistema.mapa);
        }

        sistema
    }

    pub fn clusters_geometricos(&self) -> UnionFind {
        let n = self.elementos.len();
        let mut uf = UnionFind::new(n);

        for (pos, celda) in self.elementos.iter().enumerate() {
            for &vecino in &celda.veclist {
                if celda.estado == self.elementos[vecino].estado {
                    uf.union(pos, vecino);
                }
            }
        }

        uf
    }

    pub fn clusters_fkw<R: Rng>(&self, rng: &mut R) -> UnionFind {
        let n = self.elementos.len();
        let mut uf = UnionFind::new(n);
        let beta = 1.0 / self.temp;
        let p = 1.0 - (-2.0 * beta * self.j).exp();

        for (pos, celda) in self.elementos.iter().enumerate() {
            for &vecino in &celda.veclist {
                if celda.estado == self.elementos[vecino].estado {
                    let r: f64 = rng.random();
                    if r < p {
                        uf.union(pos, vecino);
                    }
                }
            }
        }

        uf
    }

    pub fn campo_local(&self,celda: &Celda) -> f64 {
        let mut suma = 0.0;
        for pos in celda.veclist.iter() {

            if let Some(vcelda) = self.elementos.get(*pos) {
                suma = suma + vcelda.spin();
            }
        }

        suma*self.j + self.h
    }

    pub fn tamanios_clusters(&self, uf: &mut UnionFind) -> Vec<usize> {
        let n = self.elementos.len();

        let mut conteo: HashMap<usize, usize> = HashMap::new();

        for i in 0..n {
            let raiz = uf.find(i);
            *conteo.entry(raiz).or_insert(0) += 1;
        }

        conteo.into_values().collect()
    }

    pub fn magnetizacion(&self) -> f64 {
        let mut suma = 0.0;
        for celda in self.elementos.iter() {
            suma = suma + celda.spin()
        }

        suma / self.elementos.len() as f64
    }

    pub fn c1(&self) -> f64 {
        let mut suma = 0.0;
        let mut pares = 0;
        for celda in self.elementos.iter() {
            let mut suma_parcial = 0.0;
            for pos in celda.veclist.iter() {
                if let Some(vcelda) = self.elementos.get(*pos) {
                    suma_parcial = suma_parcial + vcelda.spin();
                    pares = pares + 1;
                }
            }
            suma = suma + suma_parcial * celda.spin();
        }

        if pares == 0 {
            0.0
        } else {
            suma / pares as f64
        }
    }

    pub fn magnetizacion_local(&self, frontera: &HashSet<usize>) -> (usize,f64) {
        let mut suma_local = 0.0;
        for &celda in frontera {
            suma_local = suma_local + self.elementos[celda].spin()
        };

        (frontera.len(),suma_local)
    }

    pub fn vecindad_local(&self, frontera: &HashSet<usize>, visitados: &mut HashSet<usize>) -> Option<HashSet<usize>> {
        let mut siguiente = HashSet::new();

        for &celda in frontera {
            for vecino in &self.elementos[celda].veclist {
                if visitados.insert(*vecino) {
                    siguiente.insert(*vecino);
                }
            }
        }

        if siguiente.is_empty() {
            None
        } else {
            Some(siguiente)
        }
    }

    pub fn generatriz(&self, origen: &HashSet<usize>, r: usize) -> HashSet<usize> {
        let mut visitados = origen.clone();
        let mut frontera = origen.clone();

        for _ in 0..r {
            match self.vecindad_local(&frontera, &mut visitados) {
                Some(siguiente) => frontera = siguiente,
                None => return frontera
            }
        };

        frontera
    }

    pub fn correlacion_local(&self, frontera: &HashSet<usize>, origen: &HashSet<usize>) -> (usize,f64) {
        let (lf,mf) = self.magnetizacion_local(frontera);
        let (lo,mo) = self.magnetizacion_local(origen);

        let pares = lf*lo;

        if pares == 0 {
            (0, 0.0)
        } else {
            let c = mf * mo / pares as f64;
            (pares as usize, c)
        }
    }

    pub fn correlacion_local_r(&self, origen: HashSet<usize>, r: usize) -> (usize, f64) {

        let frontera = self.generatriz(&origen, r);
        self.correlacion_local(&frontera, &origen)
    }

    pub fn correlaciones_local_r(&self, origen: HashSet<usize>, r: usize) -> Vec<(usize, f64)> {
        let mut visitados = origen.clone();
        let mut frontera = origen.clone();

        let mut resultados = Vec::with_capacity(r);

        for _ in 0..r {
            let siguiente = match self.vecindad_local(&frontera, &mut visitados) {
                Some(siguiente) => siguiente,
                None => break,
            };

            frontera = siguiente;

            let resultado = self.correlacion_local(&frontera, &origen);

            resultados.push(resultado);
        }

        resultados
    }

    pub fn correlacion_global_r(&self, r: usize) -> (usize, f64) {
        let mut suma = 0.0;
        let mut pares = 0;

        for celda in 0..self.elementos.len() {
            let mut origen = HashSet::new();
            origen.insert(celda);

            let (n,c) = self.correlacion_local_r(origen, r);

            suma = suma + c*n as f64;
            pares = pares + n;
        }

        if pares == 0 {
            return (0, 0.0);
        } else {
            (pares, suma / pares as f64)
        }
    }

    pub fn correlaciones_global_r(&self, r: usize) -> Vec<(usize, f64)> {
        let mut sumas = vec![0.0; r];
        let mut pares = vec![0usize; r];

        for celda in 0..self.elementos.len() {
            let mut origen = HashSet::new();
            origen.insert(celda);

            let resultados = self.correlaciones_local_r(origen, r);

            for (ri, &(n, c)) in resultados.iter().enumerate() {
                sumas[ri] = sumas[ri] + c*n as f64;
                pares[ri] = pares[ri] + n;
            }
        }

        let mut resultado = Vec::with_capacity(pares.len());

        for ri in 0..pares.len() {
            if pares[ri] == 0 {
                resultado.push((0, 0.0));
            } else {
                resultado.push((pares[ri], sumas[ri] / pares[ri] as f64))
            }
        };

        resultado
    }

    pub fn fotografia(&self) -> String {
        let mut eactivo = Estado::Positivo;
        let mut foto = String::new();
        for (pos,celda) in self.elementos.iter().enumerate() {
            if celda.estado != eactivo {
                write!(&mut foto, "{} ", pos).unwrap();
                eactivo.flip();
            }
        }

        foto
    }

    pub fn glauber<R: Rng>(&mut self, pos: usize, rng: &mut R) -> Result<(), Box<dyn Error>> {
        let beta = 1.0/self.temp;

        let campo = {
            match self.elementos.get(pos) {
                Some(celda) => {
                    self.campo_local(celda)
                },
                None => return Err("No existe la celda".into())
            }
        };

        let celda = &mut self.elementos[pos];
        
        let expfactor = (-2.0*beta*campo).exp();
        let pplus = 1.0/(1.0 + expfactor);

        let randp: f64 = rng.random();
        if randp < pplus {
            celda.set_state(Estado::Positivo);
        } else {
            celda.set_state(Estado::Negativo);
        }

        Ok(())
    }

    pub fn metropolis<R: Rng>(&mut self, pos: usize, rng: &mut R) -> Result<(), Box<dyn Error>> {
        let beta = 1.0/self.temp;

        let campo = {
            match self.elementos.get(pos) {
                Some(celda) => {
                    self.campo_local(celda)
                },
                None => return Err("No existe la celda".into())
            }
        };

        let celda = &mut self.elementos[pos];

        let delta = 2.0*campo*celda.spin();
        let expfactor = (-1.0*beta*delta).exp();
        let pacc = expfactor.min(1.0);

        let randp: f64 = rng.random();
        if randp < pacc {
            celda.flip();
        };

        Ok(()) 

    }

    pub fn sweep<R: Rng>(&mut self, rng: &mut R, dinamica: &Dinamica) -> Result<(), Box<dyn Error>> {
        for _ in 0..self.elementos.len() {
            let pos = rng.random_range(0..self.elementos.len());
            match dinamica {
                Dinamica::Glauber => self.glauber(pos, rng)?,
                Dinamica::Metropolis => self.metropolis(pos, rng)?,
            }
        };

        Ok(())
    }

    pub fn escribir_resumen<W: Write>(&self, archivo: &mut W) -> io::Result<()> {
        writeln!(archivo,"N,J,H,T")?;

        let n = self.elementos.len();
        let j = self.j;
        let h = self.h;
        let temp = self.temp;

        writeln!(archivo,"{},{},{},{}",n,j,h,temp)?;

        Ok(())
    }

    pub fn escribir_mapa<W: Write>(&self, archivo: &mut W) -> io::Result<()> {
        for (pos,celda) in self.elementos.iter().enumerate() {
            writeln!(archivo, "{} {}", pos, celda.id)?;
        };

        Ok(())
    }

    pub fn escribir_red<W: Write>(&self, archivo: &mut W) -> io::Result<()> {
        for celda in &self.elementos {
            for vecino in &celda.vecinos {
                writeln!(archivo, "{} {}", celda.id, vecino)?;
            }
        }

        Ok(())
    }
}

#[allow(dead_code)]
pub enum Dinamica {
    Glauber,
    Metropolis
}

#[allow(dead_code)]
pub enum Inicial {
    Random,
    Positivo,
    Negativo,
    Parcial(f32)
}

#[allow(dead_code)]
pub struct Celda {
    id: String,
    vecinos: Vec<String>,
    veclist: Vec<usize>,
    estado: Estado
}

impl Celda {
    pub fn new(id: &str, estado: Estado) -> Self {
        Celda {
            id: id.to_string(),
            vecinos: Vec::new(),
            veclist: Vec::new(),
            estado: estado
        }
    }

    pub fn add_vecino(&mut self,idv: String) {
        self.vecinos.push(idv);
    }

    pub fn seguir_vecinos(&mut self, mapa: &HashMap<String,usize>) {
        for id in self.vecinos.iter() {
            let posicion = mapa.get(id);

            if let Some(pos) = posicion {
                self.veclist.push(*pos);
            }
        }
    }

    pub fn set_state(&mut self,estado: Estado) {
        self.estado = estado;
    }

    pub fn flip(&mut self) {
        self.estado.flip();
    }

    pub fn spin(&self) -> f64 {
        self.estado.spin()
    }
}

#[derive(PartialEq)]
pub enum Estado {
    Positivo,
    Negativo
}

impl Estado {
    pub fn spin(&self) -> f64 {
        match self {
            Estado::Positivo => 1.0,
            Estado::Negativo => -1.0
        }
    }

    pub fn flip(&mut self) {
        *self = match self {
            Estado::Negativo => Estado::Positivo,
            Estado::Positivo => Estado::Negativo
        }
    }
}

impl Distribution<Estado> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Estado {
        let index = rng.random_range(0..2);
        match index {
            0 => Estado::Positivo,
            1 => Estado::Negativo,
            _ => unreachable!()
        }
    }
}

pub struct UnionFind {
    padre: Vec<usize>,
    rango: Vec<u8>
}

impl UnionFind {
    pub fn new(n: usize) -> Self {
        UnionFind { padre: (0..n).collect(), rango: vec![0;n] }
    }

    pub fn find(&mut self, x: usize) -> usize {
        if self.padre[x] != x {
            self.padre[x] = self.find(self.padre[x]);
        }
        self.padre[x]
    }

    pub fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {return;}

        if self.rango[ra] < self.rango[rb] {
            self.padre[ra] = rb;
        } else if self.rango[ra] > self.rango[rb] {
            self.padre[rb] = ra;
        } else {
            self.padre[rb] = ra;
            self.rango[ra] += 1;
        }
    }
}