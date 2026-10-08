# Water CPU optimization measurements

Source `2cad22a561` passes 57 focused water tests and all 15 RTX 4080
fixed views, each with 96 valid steady GPU samples. Exactly empty ripple
fields skip their kernel while keeping clock phase. Spectrum integrals
are cached independently of time and swell gain.

Source `cba15c8423` includes the same behavior and passes all eight browser
cases on WebGPU and WebGL2. Water Lab WebGPU averages 2.065 ms of GPU time
and 0.300 ms of main-thread elapsed time; Everglade averages 0.369 ms and
0.062 ms respectively. Browser elapsed time is not thread CPU time.
WebGL2 has no GPU timestamps. The harness-derived WASM is not a production
artifact. Captures remain outside Git in the scratch paths in the receipt.

The Mac Low pond-posts run fails the sample-validity requirement. Its
surface end counters often precede their starts and sometimes repeat an
older end counter. Those samples are rejected; the retained passing
subset must not establish a GPU budget. The raw counters and failed test
remain in this bundle. Mac timer diagnosis and budget calibration remain
open. No budget constants have been raised to hide a failure.
