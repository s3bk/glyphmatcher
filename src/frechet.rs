use itertools::Itertools;
use pathfinder_geometry::{vector::Vector2F};
use pathfinder_content::outline::{Contour, Outline};

use crate::{max, min};

fn euclidean_distance(a: Vector2F, b: Vector2F) -> f32 {
    (a - b).length()
}

fn curve_length(contour: &Contour) -> f32 {
    contour.points().iter().cloned().tuple_windows().map(|(a, b)| euclidean_distance(a, b)).sum()
}

fn extend_point_on_line(a: Vector2F, b: Vector2F, dist: f32) -> Vector2F {
    let norm = dist / euclidean_distance(a, b);
    b + (a - b) * norm
}

fn calc_value(i: usize, j: usize, prev_results_col: &[f32], current_results_col: &[f32], long_curve: &Contour, short_curve: &Contour) -> f32 {
    let long_curve = long_curve.points();
    let short_curve = short_curve.points();

    if i == 0 && j == 0 {
        return euclidean_distance(long_curve[0], short_curve[0]);
    }
    if i > 0 && j == 0 {
        return max(prev_results_col[0], euclidean_distance(long_curve[i], short_curve[0]));
    }
    let last_result = current_results_col[current_results_col.len() - 1];
    if i == 0 && j > 0 {
        return max(last_result, euclidean_distance(long_curve[0], short_curve[j]));
    }
    max(
        min(min(prev_results_col[j], prev_results_col[j - 1]), last_result),
        euclidean_distance(long_curve[i], short_curve[j])
    )
}

pub fn frechet_distance(curve1: &Contour, curve2: &Contour) -> f32 {
    let longcalcurve;
    let shortcalcurve;
    if curve1.points().len() > curve2.points().len() {
        longcalcurve = curve1;
        shortcalcurve = curve2
    } else {
        shortcalcurve = curve1;
        longcalcurve = curve2;
    }

    let mut prev_resultscalcol = vec![];
    for i in 0 .. longcalcurve.points().len() as usize {
        let mut current_resultscalcol = vec![];
        for j in 0 .. shortcalcurve.points().len() as usize {
            current_resultscalcol.push(
                calc_value(
                    i, j, &prev_resultscalcol, 
                    &current_resultscalcol, 
                    longcalcurve, shortcalcurve
                )
            );
        }
        prev_resultscalcol = current_resultscalcol;
    }
    prev_resultscalcol[shortcalcurve.points().len() as usize - 1]
}
