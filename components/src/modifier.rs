use num_traits::Num;

use eh_mod_dev::schema::schema::ComponentStats;

#[derive(Debug, Clone, Default)]
pub struct StatsModifier {
    pub armor_points: Modifier<f32>,
    pub armor_repair_rate: Modifier<f32>,
    pub armor_repair_cooldown_modifier: Modifier<f32>,
    pub energy_points: Modifier<f32>,
    pub energy_recharge_rate: Modifier<f32>,
    pub energy_recharge_cooldown_modifier: Modifier<f32>,
    pub shield_points: Modifier<f32>,
    pub shield_recharge_rate: Modifier<f32>,
    pub shield_recharge_cooldown_modifier: Modifier<f32>,
    pub weight: Modifier<f32>,
    pub ramming_damage: Modifier<f32>,
    pub energy_absorption: Modifier<f32>,
    pub kinetic_resistance: Modifier<f32>,
    pub energy_resistance: Modifier<f32>,
    pub thermal_resistance: Modifier<f32>,
    pub engine_power: Modifier<f32>,
    pub turn_rate: Modifier<f32>,
    pub drone_range_modifier: Modifier<f32>,
    pub drone_damage_modifier: Modifier<f32>,
    pub drone_defense_modifier: Modifier<f32>,
    pub drone_speed_modifier: Modifier<f32>,
    pub drones_built_per_second: Modifier<f32>,
    pub drone_build_time_modifier: Modifier<f32>,
    pub weapon_fire_rate_modifier: Modifier<f32>,
    pub weapon_damage_modifier: Modifier<f32>,
    pub weapon_range_modifier: Modifier<f32>,
    pub weapon_energy_cost_modifier: Modifier<f32>,
    pub turret_turn_speed: Modifier<f32>,
}

macro_rules! with {
    ($($field:ident),* $(,)?) => {
        $(pub fn $field(mut self, mult: impl Into<Modifier<f32>>) -> Self {
            self.$field = mult.into();
            self
        })*

        pub fn apply(&self, stats: &mut ComponentStats) {
            $(self.$field.apply_to(&mut stats.$field);)*
        }
    };
}

macro_rules! mod_impls {
    ($(
        $(#[$($attrss:tt)*])*
        $name:ident($($field:ident),* $(,)?);
    )*) => {
        $(
            $(#[$($attrss)*])*
            pub fn $name(mut self, mult: impl Into<Modifier<f32>>) -> Self {
                let mult = mult.into();
                $(self.$field = mult;)*
                self
            }
        )*
    };
}

impl StatsModifier {
    pub fn new() -> Self {
        Default::default()
    }

    mod_impls!(
        /// Defensive stats
        defensive(armor_points, shield_points, ramming_damage, energy_absorption, kinetic_resistance, energy_resistance, thermal_resistance);
        /// Resistance and related stats
        resistance(ramming_damage, energy_absorption, kinetic_resistance, energy_resistance, thermal_resistance);
        /// Defensive recovery stats
        recovery(armor_repair_rate, shield_recharge_rate);
        /// Energy points and recharge stats
        energy(energy_points, energy_recharge_rate);
        /// Engine stats
        engine(engine_power, turn_rate);
        /// Weapon boost stats
        boosts(weapon_fire_rate_modifier, weapon_damage_modifier, weapon_range_modifier, weapon_energy_cost_modifier);
        /// Drone boost stats
        drone(drone_range_modifier, drone_damage_modifier, drone_defense_modifier, drone_speed_modifier, drones_built_per_second, drone_build_time_modifier);
    );

    with!(
        armor_points,
        armor_repair_rate,
        armor_repair_cooldown_modifier,
        energy_points,
        energy_recharge_rate,
        energy_recharge_cooldown_modifier,
        shield_points,
        shield_recharge_rate,
        shield_recharge_cooldown_modifier,
        weight,
        ramming_damage,
        energy_absorption,
        kinetic_resistance,
        energy_resistance,
        thermal_resistance,
        engine_power,
        turn_rate,
        drone_range_modifier,
        drone_damage_modifier,
        drone_defense_modifier,
        drone_speed_modifier,
        drones_built_per_second,
        drone_build_time_modifier,
        weapon_fire_rate_modifier,
        weapon_damage_modifier,
        weapon_range_modifier,
        weapon_energy_cost_modifier,
        turret_turn_speed,
    );
}

#[derive(Debug, Copy, Clone, Default)]
pub enum Modifier<T: Num> {
    #[default]
    None,
    Add(T),
    Multiply(T),
    Func(fn(T) -> T),
}

impl<T: Num> Modifier<T> {
    pub fn apply(self, value: T) -> T {
        match self {
            Modifier::None => value,
            Modifier::Add(add) => value + add,
            Modifier::Multiply(times) => value * times,
            Modifier::Func(func) => func(value),
        }
    }
}

impl<T: Num + Copy> Modifier<T> {
    pub fn apply_to(self, value: &mut T) {
        match self {
            Modifier::None => (),
            Modifier::Add(add) => *value = *value + add,
            Modifier::Multiply(times) => *value = *value * times,
            Modifier::Func(func) => *value = func(*value),
        }
    }
}

impl<T: Num> From<T> for Modifier<T> {
    fn from(value: T) -> Self {
        Modifier::Multiply(value)
    }
}

impl<T: Num> From<fn(T) -> T> for Modifier<T> {
    fn from(func: fn(T) -> T) -> Self {
        Modifier::Func(func)
    }
}
