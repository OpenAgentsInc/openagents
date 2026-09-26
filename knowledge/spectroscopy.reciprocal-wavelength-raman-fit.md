---
id: spectroscopy.reciprocal-wavelength-raman-fit
version: 1
kind: method
title: Convert reciprocal wavelength axes before fitting Raman peaks
summary: >-
  When spectral x values are stored as wavelength in nanometres but peak
  positions are required as Raman shifts, transform each x value using the
  excitation wavelength before locating or fitting peaks. Fit and report peak
  center and width on the transformed axis, not the raw wavelength axis.
tags: [spectroscopy, raman, nonlinear-fit, axis-conversion]
applies_when: >-
  Code loads Raman-like spectra where the first column is wavelength and
  reported peaks are Raman shifts in inverse centimetres.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - raman-fitting
  cites:
    - IUPAC Gold Book, “Raman shift” entry
    - J. R. Lakowicz, Principles of Fluorescence Spectroscopy, 3rd ed., Springer (2006), section 9.2.2
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

For a Stokes wavelength `lambda_s` in nm and excitation wavelength `lambda_0` in nm, the Raman shift is

`nu_tilde = 10^7 * (1/lambda_0 - 1/lambda_s)` cm^-1.

If the data's first column is already a reciprocal-wavelength quantity, the corresponding conversion may simplify to a scaled reciprocal, but establish the units and convention from metadata or known spectral features rather than assuming. The conversion is nonlinear: uniform sampling in wavelength is not uniform sampling in Raman shift, and fitting a peak in the raw coordinate then transforming only its center does not preserve its width or line shape.

A common single-peak Lorentzian parameterization is `y = offset + A * gamma^2 / ((x-x0)^2 + gamma^2)`. Here `x0` is the peak center, `A` is peak height above the constant offset, and `gamma` is the half-width at half-maximum (FWHM is `2*gamma`). A constant-offset, single-Lorentzian model is only an approximation; sloping backgrounds, overlapping bands, or asymmetric/multicomponent peaks need an appropriate background or peak model. State the excitation wavelength, coordinate transformation, model, fit interval, and width convention with reported parameters.

Source: G. D. Smith and R. J. Clark, “Raman microscopy in archaeology and art history,” *Applied Spectroscopy* 61 (2007), section “Raman spectra”; J. R. Lakowicz, *Principles of Fluorescence Spectroscopy*, 3rd ed., Springer (2006), section 9.2.2 discusses Lorentzian spectral line shapes and linewidth. For the shift formula and units, see the IUPAC Gold Book, “Raman shift” entry.
