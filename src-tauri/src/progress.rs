pub fn duration_weighted_fraction(
    completed_seconds: f64,
    current_seconds: f64,
    current_fraction: f64,
    total_seconds: f64,
) -> Option<f64> {
    if !total_seconds.is_finite() || total_seconds <= 0.0 {
        return None;
    }
    let completed = completed_seconds.max(0.0);
    let current = current_seconds.max(0.0) * current_fraction.clamp(0.0, 1.0);
    Some(((completed + current) / total_seconds).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::duration_weighted_fraction;
    #[test]
    fn weights_progress_by_duration_and_clamps() {
        let actual = duration_weighted_fraction(120.0, 7200.0, 0.10, 7320.0).unwrap();
        assert!((actual - (840.0 / 7320.0)).abs() < f64::EPSILON);
        assert_eq!(duration_weighted_fraction(0.0, 1.0, 2.0, 1.0), Some(1.0));
    }
}
