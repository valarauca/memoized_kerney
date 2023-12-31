
use std::{
    sync::Arc,
    hash::{Hash,Hasher,BuildHasher},
    time::{Duration},
    mem::{swap},
};

#[macro_use] extern crate lazy_static;

use tokio::sync::RwLock;
use seahash::{SeaHasher};
use moka::future::{Cache};

/// Location stores a Lat & Lon data.
///
/// It provides a simple entry point for data entering the API and
/// ensures data entering & exiting are in a uniform format.
#[derive(Clone,Copy,Debug,PartialOrd)]
pub struct Position {
    lat: f64,
    lon: f64,
}
impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        (self.lat.to_ne_bytes() == other.lat.to_ne_bytes())
            &
        (self.lon.to_ne_bytes() == other.lon.to_ne_bytes())
    }
}
impl Eq for Position { }
impl Hash for Position {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write( self.lat.to_ne_bytes().as_ref());
        state.write( self.lon.to_ne_bytes().as_ref());
    }
}
impl Position {
    pub const fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}
impl IntoPosition for Position {
    fn get_lat(&self) -> f64 { self.lat }
    fn get_lon(&self) -> f64 { self.lon }
    fn into_position(&self) -> Position {
        self.clone()
    }
}

/// Binding type for the API
pub trait IntoPosition {
    fn get_lat(&self) -> f64;
    fn get_lon(&self) -> f64;

    fn into_position(&self) -> Position {
        Position::new(self.get_lat(), self.get_lon()) 
    }
}

/// Returns information about a distance query
///
/// Please note, this does return more than just the basic distance the
/// azimuth information is calculated as part of the InverseGeodesic problem,
/// and can be useful when performing additional calculations so it is returned
/// as part of the API as a convience. The only additional computation requriements
/// are copying ~2 extra floating point values
#[derive(Copy,Clone,PartialEq,PartialOrd,Debug)]
pub struct DistanceData {
    /// Distance from `A` to `B` in meters on the WSG84 Spheroid
    pub distance: f64,
    /// Bearing you'd have to have to reach `B` from `A`
    pub forward_azimuth: f64,
    /// Bearing you'd have to face to reach `A` from `B`
    pub backward_azimuth: f64,
}
impl DistanceData {
    /// Used because the cache stores southern most points first, to avoid caching (A->B & B->A)
    /// seperately.
    fn swap_azimuth(&mut self, should_swap: bool) {
        if should_swap {
            swap(&mut self.forward_azimuth, &mut self.backward_azimuth);
        }
    }
}

#[derive(Default,Clone,Copy)]
struct BuildSeaHasher {
    #[allow(dead_code)] _data: u8,
}
impl BuildHasher for BuildSeaHasher {
    type Hasher = SeaHasher;
    fn build_hasher(&self) -> SeaHasher {
        SeaHasher::new()
    }
}
unsafe impl Sync for BuildSeaHasher { }
unsafe impl Send for BuildSeaHasher { }

lazy_static! {
    static ref DISTANCE_CACHE: Arc<RwLock<Cache<(Position,Position),DistanceData,BuildSeaHasher>>> = {
        let cache = Cache::builder()
            .time_to_idle(Duration::from_secs(90))
            .initial_capacity(64)
            .max_capacity(65356)
            .build_with_hasher(BuildSeaHasher::default());
        Arc::new(RwLock::new(cache))
    };
}

async fn time_future<F>(arg: F) -> (<F as std::future::Future>::Output,std::time::Duration)
where
    F: std::future::Future
{
    use tokio::time::Instant;

    let now = tokio::time::Instant::now();
    let result = arg.await;
    let later = now.elapsed();
    (result,later)
}

pub fn uncached_distance<A,B>(a: &A, b: &B) -> DistanceData
where
    A: IntoPosition,
    B: IntoPosition,
{
    use geographiclib_rs::{Geodesic,InverseGeodesic};

    let a_pos = a.into_position();
    let b_pos = b.into_position();
    let flip = a_pos > b_pos;
    let tup = if flip {
        (b_pos, a_pos)
    } else {
        (a_pos, b_pos)
    };

    let wgs84 = Geodesic::wgs84();
    let (s12, azi_1, azi_2, _): (f64,f64,f64,f64) = wgs84.inverse(tup.0.get_lat(), tup.0.get_lon(), tup.1.get_lat(), tup.1.get_lon());

    let mut dist = DistanceData {
        distance: s12,
        forward_azimuth: azi_1,
        backward_azimuth: azi_2,
    };
    dist.swap_azimuth(flip);
    dist
}

/// calculte the distance between 2 points
pub async fn distance<A,B>(a: &A, b: &B) -> DistanceData
where
    A: IntoPosition,
    B: IntoPosition,
{
    let a_pos: Position = a.into_position();
    let b_pos: Position = b.into_position();
    let flip: bool = a_pos > b_pos;
    let tup: (Position,Position) = if flip {
        (b_pos, a_pos)
    } else {
        (a_pos, b_pos)
    };
 
    match DISTANCE_CACHE.read().await.get(&tup).await {
        Option::Some(mut dist) => {
            dist.swap_azimuth(flip);
            return dist;
        },
        Option::None => { }
    };
    let mut dist = uncached_distance(&tup.0, &tup.1);
    DISTANCE_CACHE.write().await.insert(tup,dist.clone()).await;

    dist.swap_azimuth(flip);
    dist
}
