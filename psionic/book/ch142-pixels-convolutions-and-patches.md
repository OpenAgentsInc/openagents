# 142. Pixels, convolutions and patches

> **Status: partly implemented.** Image admission, preprocessing profiles,
> nearest, bilinear and bicubic resizing with half-pixel or aligned corners
> and antialiasing, resize-and-crop rules, 2-D convolution with stride,
> padding, dilation and groups (forward two ways, and its gradients), a tiled
> patch projection large enough for ViT-B/16, position-table resizing, and
> the convolution and patch projection lowered into chapter 38's tensor IR
> run on the CPU. Entries for these operators in chapter 5's operator table
> and its manifest, and their lowering to a GPU, are not built yet.

Every vision model in this book starts by turning an image file into a
tensor with exact conventions. A pixel range of `[0, 1]` instead of
`[0, 255]`, another resize filter or a swapped channel order changes every
value downstream, and the model still produces confident output. This chapter
writes those conventions down as data, builds the operations that Part XIII's
models share, and checks each one on fixtures small enough to verify by hand.

It builds seven things, in [`pixels.coax`](../lib/pixels.coax) and
[`pixels-ir.coax`](../lib/pixels-ir.coax):
- a **preprocessing profile**: channel order, value scale, per-channel mean
  and spread, output size, resize filter, corner rule, antialiasing and an
  optional resize-then-crop rule, and the procedure that turns 8-bit RGB
  bytes into a normalized channel-major tensor;
- **resizing** by nearest, bilinear and bicubic filters, each written the way
  PyTorch or PIL computes it;
- **2-D convolution** with stride, padding, dilation and groups, computed by
  a direct sum and again by image-to-columns plus
  [chapter 4](ch004-numbers-and-matrices.md)'s matrix product;
- the convolution's **gradients** for the input, the weights and the bias;
- **patch projection**, the first layer of a vision transformer, in tiles,
  so that a real ViT-B/16 projection fits chapter 4's product;
- the convolution and patch projection **lowered into
  [chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md)'s tensor
  IR**, where a compiled program runs them;
- **position-table resizing**, for running such a model at a new image size.

```shell
coax/bin/coax run psionic/examples/pixels.coax
coax/bin/coax run psionic/examples/pixels-vit.coax
coax/bin/coax run psionic/examples/pixels-refuse.coax
psionic/check.sh --only pixels pixels-ir
```

```text
2x2 to 4x4, bilinear: ok, 48 values
  red   -1.0 -0.5 0.5 1.0
  red   -0.5 -0.25 0.25 0.5
  red   0.5 0.25 -0.25 -0.5
  red   1.0 0.5 -0.5 -1.0
  green row 0 -0.6 -0.6 -0.6 -0.6
  blue row 3  -1.0 -0.5 0.5 1.0
  red row 1, nearest -1.0 -1.0 1.0 1.0
7x5 to 9x4, ImageNet profile: ok, largest difference from the reference 1.9689196945060772e-07
box sum, stride 2, padding 1: 12.5 16.5 24.5 28.5
2x2 ones, dilation 2: 20.0
32 settings: direct equals im2col on 32 of them; central differences equal the gradients on 32 of them
sevenths and ninths: 13 of 30 outputs differ, by at most 1.1920928955078125e-07
patches as rows: 1.0 2.0 5.0 6.0 3.0 4.0 7.0 8.0
projected tokens: 1.0 14.5 3.0 22.5
stride-2 convolution, (filter, column): 1.0 3.0 14.5 22.5
  positions 0.0 0.25 0.75 1.0
  positions 0.25 0.375 0.625 0.75
  positions 0.75 0.625 0.375 0.25
  positions 1.0 0.75 0.25 0.0
```

The [example](../examples/pixels.coax) runs in about four seconds, nearly all
of it in the central differences.

## Image tensors and layouts

An image arrives as bytes: row after row, each pixel's channels together.
That layout is (height, width, channel), often written HWC. Models want
(channel, height, width), CHW, with each channel a contiguous plane, so that a
convolution or patch reads one channel's neighbourhood from one place. A batch
of images adds a leading axis: (batch, channel, height, width), or NCHW.

Pixel (y, x) of channel c in an H by W image sits at byte `(y·W + x)·3 + c`
in the input and at index `(c·H + y)·W + x` in the tensor.

The types below describe every input this chapter accepts. Each one's counts
are checked before any value is read.

<!-- source: psionic/lib/pixels.coax types -->
```jai
PxStatus :: enum u8 {ok;shape;channels;depth;limit;parameter;capacity;alias;number;}
PxOrder :: enum u8 {rgb;bgr;}
PxFilter :: enum u8 {nearest;bilinear;bicubic;}
// Pixels arrive row by row, each pixel's channels together (height, width,
// channel), one byte per channel, in the stated channel order.
PxImage :: struct {width:int;height:int;channels:int=3;depth:int=8;order:PxOrder;}
// What a model expects: channel order, the factor that maps a byte to its
// range, per-channel mean and standard deviation, and the resize rule: the
// filter, corner alignment, antialiasing, and an optional shorter side to
// resize to before a centred crop of height by width.
PxProfile :: struct {order:PxOrder;scale:f64;mean:[3]f64;std:[3]f64;height:int;width:int;filter:PxFilter;
    corners:bool;antialias:bool;shorter:int;}
// The resized grid a profile samples, and where its crop starts.
PxFit :: struct {status:PxStatus;height:int;width:int;top:int;left:int;}
// The source pixels one output reads along one axis, and their weights.
PxTaps :: struct {first:int;count:int;}
PxResult :: struct {status:PxStatus;written:int;}
// Input (batch, channels, height, width); weights (filters, channels/groups,
// kernel_h, kernel_w); output (batch, filters, out_h, out_w).
PxConv :: struct {batch:int=1;channels:int;height:int;width:int;filters:int;kernel_h:int;kernel_w:int;
    stride_h:int=1;stride_w:int=1;pad_h:int;pad_w:int;dilation_h:int=1;dilation_w:int=1;groups:int=1;}
PxConvPlan :: struct {status:PxStatus;out_h:int;out_w:int;x:int;w:int;y:int;patch:int;columns:int;work:int;scratch:int;}
// Input (batch, channels, height, width); weights (dim, channels, patch,
// patch), the layout of the equivalent convolution; output (batch, tokens, dim).
PxPatch :: struct {batch:int=1;channels:int;height:int;width:int;patch:int;dim:int;}
PxPatchPlan :: struct {status:PxStatus;grid_h:int;grid_w:int;tokens:int;features:int;x:int;w:int;out:int;work:int;rows:int;scratch:int;}
```

## A preprocessing profile

A model's preprocessing is part of the model. Its training code fixed:
- the **channel order**: RGB, or BGR for models trained on images decoded
  with OpenCV;
- the **scale** that maps a byte to the model's range, 1/255 for `[0, 1]`;
- the **mean** subtracted from each channel and the **spread** each is
  divided by, after scaling;
- the **output size** and the **resize filter**.

The profile record holds exactly those fields, plus three that refine the
resize: whether the corners are aligned, whether downscaling is antialiased,
and an optional shorter side to resize to before a centred crop. Admission refuses an image
with a zero side, a side above 512, any channel count other than three or a
depth other than 8 bits, each with its own status. It refuses a profile whose
scale is outside (0, 1], whose mean is outside [−256, 256] or whose spread is
outside [2⁻¹⁰, 256]. NaN fails every one of those comparisons, so it is
refused too. A shorter side below 0 is refused, and one above 512 is over
the limit. Antialiasing is refused for the nearest filter, which has no
width to widen, and together with aligned corners, a combination neither
PIL nor PyTorch defines.

<!-- source: psionic/lib/pixels.coax profiles -->
```jai
// ImageNet's statistics, as most convolutional and many transformer image
// models were trained with them.
px_imagenet :: ()->PxProfile {
    return PxProfile.{scale=1.0/255,mean=[3]f64.[0.485,0.456,0.406],std=[3]f64.[0.229,0.224,0.225],height=224,width=224,filter=PxFilter.bilinear};
}
px_image_status :: (image:PxImage)->PxStatus {
    if image.width<1 || image.height<1 {return PxStatus.shape;}
    if image.width>PX_SIDE_MAX || image.height>PX_SIDE_MAX {return PxStatus.limit;}
    if image.channels!=3 {return PxStatus.channels;}
    if image.depth!=8 {return PxStatus.depth;}
    return PxStatus.ok;
}
px_profile_status :: (p:PxProfile)->PxStatus {
    if p.width<1 || p.height<1 {return PxStatus.parameter;}
    if p.width>PX_SIDE_MAX || p.height>PX_SIDE_MAX {return PxStatus.limit;}
    if !(p.scale>0 && p.scale<=1) {return PxStatus.parameter;}
    for c:0..<3 {
        if !(p.mean[c]>=-PX_MEAN_MAX && p.mean[c]<=PX_MEAN_MAX) || !(p.std[c]>=PX_STD_MIN && p.std[c]<=PX_MEAN_MAX) {return PxStatus.parameter;}
    }
    if p.shorter<0 {return PxStatus.parameter;}
    if p.shorter>PX_SIDE_MAX {return PxStatus.limit;}
    // Antialiasing widens a filter; nearest has none, and the widened filter
    // is defined for half-pixel centres only.
    if p.antialias && (p.filter==PxFilter.nearest || p.corners) {return PxStatus.parameter;}
    return PxStatus.ok;
}
```

## Resizing by hand

Resizing reads each output pixel from a point in the source. Chapter 5's
bilinear operator places pixel centres at half-integers: output pixel o of
`size` maps to source point `(o + 1/2) · in / size − 1/2`, clamped to the
image. That is the rule PyTorch calls `align_corners=False`. Nearest
sampling reads source pixel `floor(o · in / size)`.

Take a 2 by 2 image and resize it to 4 by 4. Along one axis, in = 2 and
size = 4:

| Output o | Source point | Clamped | Neighbours, fraction |
| --- | --- | --- | --- |
| 0 | (0.5)·2/4 − 0.5 = −0.25 | 0 | 0 and 1, 0 |
| 1 | (1.5)·2/4 − 0.5 = 0.25 | 0.25 | 0 and 1, 0.25 |
| 2 | (2.5)·2/4 − 0.5 = 0.75 | 0.75 | 0 and 1, 0.75 |
| 3 | (3.5)·2/4 − 0.5 = 1.25 | 1 | 1 and 1, 0 |

The fixture's red channel is 0 255 / 255 0. Scaled by 1/255 that is
0 1 / 1 0. The top source row resizes to 0, 0.25, 0.75, 1; the bottom row to
1, 0.75, 0.25, 0. Output row 1 sits at source row 0.25, so it is
0.75 · top + 0.25 · bottom = 0.25, 0.375, 0.625, 0.75. The profile in the
example subtracts 0.5 and divides by 0.5, which maps v to 2v − 1. Row 1
becomes −0.5, −0.25, 0.25, 0.5, which is what the program prints.

Green is 51 everywhere: 51/255 = 0.2, and (0.2 − 0.5)/0.5 = −0.6. Each
resized value is a blend of equal neighbours, so every green output is −0.6
after one rounding to f32. Nearest sampling repeats each source row twice:
rows 0, 0, 1, 1, so red row 1 is −1, −1, 1, 1.

## The resize and the preprocessing

`px_sample` computes one resized value with chapter 5's arithmetic. For
bilinear it calls chapter 5's `to_point`, which finds the source point in
exact integer arithmetic (low neighbour, high neighbour and the fraction
between them), then blends the four neighbours in binary64. It reads its grid
through a base offset and a row and column step, so the same procedure serves
an image plane and a position table.

<!-- source: psionic/lib/pixels.coax sample -->
```jai
// One resized value, with chapter 5's arithmetic for nearest2d and
// bilinear2d. Nearest reads source floor(out * in / size). Bilinear takes
// chapter 5's half-pixel source point (to_point: low and high neighbours and
// the fraction between them) on each axis and blends the four neighbours.
// The grid is read at base + r * row + c * column, so one procedure serves a
// channel-major image plane and a position table with features innermost.
px_sample :: (data:[]f32,base:int,row:int,column:int,h:int,w:int,size_h:int,size_w:int,filter:PxFilter,y:int,x:int)->f64 {
    if filter==PxFilter.nearest {return f64(data[base+(y*h/size_h)*row+(x*w/size_w)*column]);}
    a:=to_point(y,h,size_h);b:=to_point(x,w,size_w);
    top:=(1-b.fraction)*f64(data[base+a.low*row+b.low*column])+b.fraction*f64(data[base+a.low*row+b.high*column]);
    bottom:=(1-b.fraction)*f64(data[base+a.high*row+b.low*column])+b.fraction*f64(data[base+a.high*row+b.high*column]);
    return (1-a.fraction)*top+a.fraction*bottom;
}
```

`px_preprocess` lays the bytes out channel by channel in the model's order,
scales them to f32, resizes each plane (cropping when the profile names a
shorter side) and normalizes. Each stage rounds to
f32 once, as chapter 5's operators would. The checks run chapter 5's
`nearest2d` and `bilinear2d` on the same scaled planes and require every
output to be identical.

<!-- source: psionic/lib/pixels.coax preprocess -->
```jai
px_preprocess_work :: (image:PxImage,p:PxProfile)->int {return 3*image.width*image.height;}
// Bytes to model values: pick the model's channel order and lay the image out
// channel by channel, times the scale; resize, and crop when the profile names
// a shorter side; subtract the mean and divide by the spread. Each stage
// rounds to f32 once, as chapter 5's operators do.
px_preprocess :: (image:PxImage,pixels:[]u8,p:PxProfile,work:[]f32,out:[]f32,overlap:bool)->PxResult {
    s:=px_image_status(image);if s!=PxStatus.ok {return PxResult.{status=s};}
    s=px_profile_status(p);if s!=PxStatus.ok {return PxResult.{status=s};}
    h:=image.height;w:=image.width;area:=h*w;size:=p.height*p.width;
    fit:=px_fit(h,w,p);if fit.status!=PxStatus.ok {return PxResult.{status=fit.status};}
    if pixels.count!=3*area {return PxResult.{status=PxStatus.shape};}
    if work.count<3*area || out.count<3*size {return PxResult.{status=PxStatus.capacity};}
    if overlap {return PxResult.{status=PxStatus.alias};}
    for c:0..<3 {
        // Model channel c is source channel c, or 2 - c when the orders differ.
        source:=c;if image.order!=p.order {source=2-c;}
        for i:0..<area {work[c*area+i]=f32(f64(pixels[i*3+source])*p.scale);}
    }
    for c:0..<3 {for y:0..<p.height {for x:0..<p.width {
        v:=f32(px_resample(work,c*area,w,1,h,w,fit.height,fit.width,p,y+fit.top,x+fit.left));
        out[(c*p.height+y)*p.width+x]=f32((f64(v)-p.mean[c])/p.std[c]);
    }}}
    return PxResult.{written=3*size};
}
```

## Three more resize rules

Libraries disagree on resizing in three ways, and a model expects the one it
was trained with.

**Where the source point is.** Half-pixel centres map output o to
`(o + 1/2) · in / size − 1/2`. With **aligned corners** (PyTorch's
`align_corners=True`) the first and last pixels of both grids coincide, and o
maps to `o · (in − 1) / (size − 1)`; one output pixel maps to 0. For 2 pixels
resized to 4 the points are 0, 1/3, 2/3 and 1 instead of −1/4, 1/4, 3/4 and
5/4. On the example's red channel, 0 1 / 1 0, output row 1 sits at source row
1/3, so it is 2/3 of the top row plus 1/3 of the bottom row:
1/3, 4/9, 5/9, 2/3. The profile maps v to 2v − 1, so the example prints
−1/3, −1/9, 1/9 and 1/3, each rounded once to f32. Nearest sampling with
aligned corners takes the nearest of those points, halves rounding up.

**Which filter.** Bicubic interpolation weighs four neighbours with Keys'
cubic kernel, which has one parameter, a:

```text
W(t) = (a + 2)|t|^3 − (a + 3)|t|^2 + 1      for |t| < 1
W(t) = a|t|^3 − 5a|t|^2 + 8a|t| − 4a       for 1 ≤ |t| < 2
W(t) = 0                                   otherwise
```

PyTorch's `interpolate` uses a = −0.75; PIL, and PyTorch with
`antialias=True`, use a = −0.5. This chapter follows them: a = −0.75 without
antialiasing, a = −0.5 with it. Take 2 pixels to 4 again. Output 1 sits at
source point 1/4, between pixel 0 (distance 1/4) and pixel 1 (distance 3/4);
the outer taps are pixels −1 and 2, at distances 5/4 and 7/4. With
a = −0.75 the weights are W(5/4) = −0.10546875, W(1/4) = 0.87890625,
W(3/4) = 0.26171875 and W(7/4) = −0.03515625. Pixel −1 is outside the image,
so its weight joins pixel 0's, and pixel 2's joins pixel 1's: 0.7734375 and
0.2265625. Output 0, at −1/4, gets 1.10546875 and −0.10546875. On the red
channel, output (1, 0) is 0.7734375 · −0.10546875 + 0.2265625 · 1.10546875
= 0.16888427734375, and 2v − 1 = −0.6622314453125, the first value of the
example's bicubic row. The corners go below −1: the kernel's negative lobes
overshoot at an edge, which bilinear sampling never does.

**Antialiasing.** Downscaling by bilinear or bicubic sampling reads only the
few pixels nearest each point and skips the rest. PIL's resampler widens the
filter by the downscaling factor instead, so that every source pixel
contributes. Output o is centred on `(o + 1/2) · in / size`; the filter is
stretched by `max(in / size, 1)`, cut at the image's edge, and the weights
are divided by their sum. Halve the row 0 0 255 255: output 0 is centred on
1, the triangle filter is twice as wide, and pixels 0, 1 and 2 sit at
distances 1/4, 1/4 and 3/4 of its half-width, so their weights are 3/4, 3/4
and 1/4 of a sum of 7/4: 3/7, 3/7 and 1/7. The output is 1/7 and 6/7, where
plain bilinear sampling gives 0 and 1.

`px_taps` returns the pixels one output reads along one axis and their
weights, under all three rules. Repeated edge pixels add their weights, so
the taps are always one contiguous run. The kernel is one cubic plus two
truncated powers, which is the formula above without its comparisons.
`px_resample` multiplies the two axes' taps; it leaves the half-pixel
nearest and bilinear rules to `px_sample`, so they stay identical to
chapter 5's operators.

<!-- source: psionic/lib/pixels.coax taps -->
```jai
// Keys' cubic convolution kernel with coefficient a, as one cubic plus two
// truncated powers: (a + 2)t^3 - (a + 3)t^2 + 1 up to t = 1, plus
// (t - 1)^2 (-2t - 4a - 1) from 1, minus a(t - 1)(t - 2)^2 from 2, where it
// cancels to 0. (u + |u|) / 2 is u above 0 and 0 below.
px_cubic :: (x:f64,a:f64)->f64 {
    t:=abs(x);r:=(t-1+abs(t-1))/2;s:=(t-2+abs(t-2))/2;
    return ((a+2)*t-(a+3))*t*t+1+r*r*(-2*t-4*a-1)-s*s*a*(t-1);
}
// i moved into 0 to n - 1.
px_clamp :: (i:int,n:int)->int {low:=(i+tv_magnitude(i))/2;return (low+n-1-tv_magnitude(low-n+1))/2;}
// The source pixels output o reads along an axis of input pixels resized to
// size, from first, with their weights. Three rules:
// - nearest reads floor(o * input / size), or with corners aligned the
//   nearest of o * (input - 1) / (size - 1), halves rounding up;
// - antialiased bilinear and bicubic follow PIL's resampler (and PyTorch's
//   antialias=True): centre (o + 1/2) * input / size, the filter widened by
//   the downscaling factor, the window cut at the image edge and the weights
//   divided by their sum; bicubic uses a = -0.5;
// - otherwise bilinear and bicubic follow PyTorch's interpolate: the exact
//   source point (o + 1/2) * input / size - 1/2, or o * (input - 1) /
//   (size - 1) with corners aligned, then 2 or 4 neighbours whose indices
//   stop at the edges; bilinear moves a point below 0 up to 0, bicubic uses
//   a = -0.75. Repeated edge pixels add their weights.
px_taps :: (input:int,size:int,filter:PxFilter,corners:bool,antialias:bool,o:int,weights:[]f64)->PxTaps {
    if filter==PxFilter.nearest {
        first:=o*input/size;
        if corners && size>1 {first=(2*o*(input-1)+size-1)/(2*(size-1));}
        weights[0]=1;return PxTaps.{first=first,count=1};
    }
    if antialias {
        // widen = max(scale, 1); the window [first, last) stops at the edges.
        scale:=f64(input)/f64(size);widen:=(scale+1+abs(scale-1))/2;
        support:=widen;if filter==PxFilter.bicubic {support=2*widen;}
        centre:=(f64(o)+0.5)*scale;inverse:=1/widen;
        first:=int(floor(centre-support+0.5));first=(first+tv_magnitude(first))/2;
        last:=int(floor(centre+support+0.5));last=(last+input-tv_magnitude(last-input))/2;
        count:=last-first;total:=0.0;
        for j:0..<count {
            u:=(f64(first+j)-centre+0.5)*inverse;v:=px_cubic(u,-0.5);
            // Twice the triangle max(1 - |u|, 0); dividing by the sum removes the 2.
            if filter==PxFilter.bilinear {v=1-abs(u)+abs(1-abs(u));}
            weights[j]=v;total+=v;
        }
        for j:0..<count {weights[j]/=total;}
        return PxTaps.{first=first,count=count};
    }
    // The source point num / den, exactly; with corners and one output the
    // point is 0 and den is 0.
    num:=(2*o+1)*input-size;den:=2*size;
    if corners {num=o*(input-1);den=size-1;}
    if filter==PxFilter.bilinear {num=(num+tv_magnitude(num))/2;}
    low:=0;t:=0.0;
    if den>0 {low=num/den;if num<low*den {low-=1;}t=f64(num-low*den)/f64(den);}
    start:=low-1;n:=4;if filter==PxFilter.bilinear {start=low;n=2;}
    first:=px_clamp(start,input);count:=px_clamp(start+n-1,input)-first+1;
    for j:0..<count {weights[j]=0;}
    for j:0..<n {
        u:=f64(start+j-low)-t;v:=px_cubic(u,-0.75);
        if filter==PxFilter.bilinear {v=1-abs(u);}
        weights[px_clamp(start+j,input)-first]+=v;
    }
    return PxTaps.{first=first,count=count};
}
// One resized value. The half-pixel nearest and bilinear rules without
// antialiasing take chapter 5's arithmetic above; every other rule sums the
// two axes' taps.
px_resample :: (data:[]f32,base:int,row:int,column:int,h:int,w:int,size_h:int,size_w:int,p:PxProfile,y:int,x:int)->f64 {
    if !p.corners && !p.antialias && p.filter!=PxFilter.bicubic {return px_sample(data,base,row,column,h,w,size_h,size_w,p.filter,y,x);}
    wy:[512]f64;wx:[512]f64;
    a:=px_taps(h,size_h,p.filter,p.corners,p.antialias,y,wy[..]);b:=px_taps(w,size_w,p.filter,p.corners,p.antialias,x,wx[..]);
    t:PsionicAccumulator;
    for i:0..<a.count {for j:0..<b.count {t=psionic_ops_accumulate(t,wy[i]*wx[j]*f64(data[base+(a.first+i)*row+(b.first+j)*column]));}}
    return psionic_ops_total(t);
}
```

## Resize, then crop

ImageNet's evaluation rule does not resize straight to 224 by 224. It
resizes the shorter side to 256 pixels, keeping the aspect ratio, and cuts
the centred 224 by 224 square out of the result. torchvision rounds the
longer side down and the crop offset to the nearest integer, halves to even
(Python's `round`).

Take a 320 by 240 photo. The shorter side, 240, becomes 256; the longer side
becomes 256 · 320 / 240 = 341.33, rounded down to 341. The crop starts
(256 − 224) / 2 = 16 rows down and (341 − 224) / 2 = 58.5 columns across,
which rounds to the even 58. The resized grid is never stored: output pixel
(y, x) samples pixel (y + 16, x + 58) of the 341 by 256 grid.

<!-- source: psionic/lib/pixels.coax fit -->
```jai
// Half of d, rounded half to even, as torchvision's centre crop rounds.
px_centre :: (d:int)->int {k:=d/2;if d%2==1 && k%2==1 {k+=1;}return k;}
// Without a shorter side, the image is resized straight to height by width.
// With one, the shorter image side becomes that many pixels, the longer side
// keeps the aspect ratio rounded down (torchvision's Resize(int)), and the
// output is the centred height by width crop of that grid.
px_fit :: (h:int,w:int,p:PxProfile)->PxFit {
    if p.shorter==0 {return PxFit.{height=p.height,width=p.width};}
    short:=(h+w-tv_magnitude(h-w))/2;rh:=p.shorter*h/short;rw:=p.shorter*w/short;
    if rh>PX_EXTENT_MAX || rw>PX_EXTENT_MAX {return PxFit.{status=PxStatus.limit};}
    if p.height>rh || p.width>rw {return PxFit.{status=PxStatus.shape};}
    return PxFit.{height=rh,width=rw,top=px_centre(rh-p.height),left=px_centre(rw-p.width)};
}
```

## An independent reference

The reference below works straight from the bytes in binary64. It computes
the resized grid and the crop in floating point, as torchvision does, and the
weight of every source pixel along each axis from each rule's usual
description: the source point and its floor for bilinear sampling, PyTorch's
two cubic convolution formulas for bicubic, and PIL's filter evaluated at
every pixel's distance from the centre, rather than only inside a window, for
antialiasing. Its output value is the sum of every pixel times its two
weights. It shares no code with `px_taps`.

<!-- source: psionic/lib/pixels.coax reference -->
```jai
// One output value straight from the bytes, in binary64: the resized grid
// and crop computed in floating point as torchvision does, then each source
// pixel times its two axis weights.
px_reference :: (image:PxImage,pixels:[]u8,p:PxProfile,c:int,y:int,x:int)->f64 {
    source:=c;if image.order!=p.order {source=2-c;}
    h:=image.height;w:=image.width;size_h:=p.height;size_w:=p.width;top:=0;left:=0;
    if p.shorter>0 {
        short:=-px_most(-f64(h),-f64(w));
        size_h=int(floor(f64(p.shorter)*f64(h)/short));size_w=int(floor(f64(p.shorter)*f64(w)/short));
        top=px_round_even(f64(size_h-p.height)/2);left=px_round_even(f64(size_w-p.width)/2);
    }
    wy:[512]f64;wx:[512]f64;px_reference_axis(h,size_h,p,y+top,wy[..]);px_reference_axis(w,size_w,p,x+left,wx[..]);
    v:=0.0;
    for i:0..<h {for j:0..<w {v+=wy[i]*wx[j]*f64(pixels[(i*w+j)*3+source]);}}
    return (v*p.scale-p.mean[c])/p.std[c];
}
// Python's round(), which rounds halves to even, as torchvision calls it.
px_round_even :: (v:f64)->int {r:=floor(v);if v-r==0.5 && int(r)%2==1 {r+=1;}return int(r);}
// The weight of every source pixel along one axis for output o, written into
// weights that arrive as zeros, from each rule's usual floating-point
// description: the source point and its floor; PyTorch's cubic convolution
// formulas; PIL's filter evaluated at every pixel's distance from the
// centre rather than only inside a window, its pieces as clamped arguments.
px_reference_axis :: (input:int,size:int,p:PxProfile,o:int,weights:[]f64) {
    if p.filter==PxFilter.nearest {
        s:=floor(f64(o)*f64(input)/f64(size));
        if p.corners && size>1 {s=floor(f64(o)*f64(input-1)/f64(size-1)+0.5);}
        weights[int(s)]=1;return;
    }
    if p.antialias {
        scale:=f64(input)/f64(size);widen:=px_most(scale,1);total:=0.0;a:=-0.5;
        for j:0..<input {
            u:=abs(f64(j)+0.5-(f64(o)+0.5)*scale)/widen;
            // Bilinear: 1 - u, not below 0. Bicubic: the inner cubic at min(u, 1), where it
            // is 0, plus the outer cubic at u held between 1 and 2, where it is 0 at both ends.
            v:=px_most(1-u,0);
            if p.filter==PxFilter.bicubic {
                i:=-px_most(-u,-1);e:=px_most(-px_most(-u,-2),1);
                v=(a+2)*i*i*i-(a+3)*i*i+1+a*e*e*e-5*a*e*e+8*a*e-4*a;
            }
            weights[j]=v;total+=v;
        }
        for j:0..<input {weights[j]/=total;}
        return;
    }
    s:=(f64(o)+0.5)*f64(input)/f64(size)-0.5;
    if p.corners {s=0;if size>1 {s=f64(o)*f64(input-1)/f64(size-1);}}
    if p.filter==PxFilter.bilinear {
        s=px_most(s,0);i0:=int(floor(s));f:=s-f64(i0);
        weights[i0]+=1-f;weights[px_clamp(i0+1,input)]+=f;return;
    }
    i0:=int(floor(s));t:=s-f64(i0);a:=-0.75;
    near:=[2]f64.[t,1-t];far:=[2]f64.[t+1,2-t];
    for k:0..<2 {
        // cubic_convolution1 for the two inner taps, cubic_convolution2 for the outer two.
        inner:=((a+2)*near[k]-(a+3))*near[k]*near[k]+1;
        outer:=((a*far[k]-5*a)*far[k]+8*a)*far[k]-4*a;
        weights[px_clamp(i0+k,input)]+=inner;weights[px_clamp(i0-1+3*k,input)]+=outer;
    }
}
// The larger of two values.
px_most :: (a:f64,b:f64)->f64 {return (a+b+abs(a-b))/2;}
```

On a 7 by 5 image resized to 9 by 4 under ImageNet's profile, the largest
difference between `px_preprocess` and the reference is 1.97 × 10⁻⁷. The f32
roundings of the scaled byte, the resized value and the normalized value
account for it: one f32 step at 2 is 2.4 × 10⁻⁷. The checks require at most
2 × 10⁻⁶ under every rule: both filters at half-pixel centres, nearest,
bilinear and bicubic with aligned corners, antialiased bilinear and bicubic,
and bilinear after a shorter side, on wide and tall images, both channel
orders, output sides of 1 and shorter sides of 1. The hand-worked weights
above are checked exactly.

## Convolution

A 2-D convolution slides a kernel over the image. Output pixel (oh, ow) of
filter f is a sum over every input channel of f's group and every kernel
tap (i, j):

```text
y[n, f, oh, ow] = bias[f] + Σ x[n, ch, oh·stride − pad + i·dilation, ow·stride − pad + j·dilation] · w[f, ch, i, j]
```

Taps that land in the padding read zero. The kernel is not flipped. This is
cross-correlation, which every deep-learning framework calls convolution.

The four settings:
- **stride** steps the window by more than one pixel;
- **padding** adds zero rows and columns around the image;
- **dilation** spaces the kernel's taps apart, so a k-tap kernel spans
  (k − 1)·dilation + 1 pixels;
- **groups** splits the channels: filter f sees only the channels of its
  group, channels/groups of them.

The output height is `(H + 2·pad − span)/stride + 1`, rounded down.

### By hand

Take the 3 by 3 image 1 to 9, a 3 by 3 kernel of ones, stride 2, padding 1
and bias 0.5. The span is 3, so the output is (3 + 2 − 3)/2 + 1 = 2 by 2.
Output (0, 0)'s window covers rows −1 to 1 and columns −1 to 1. Row −1 and
column −1 are padding, so the window sums the top-left 2 by 2 block:
1 + 2 + 4 + 5 = 12. Output (0, 1) covers columns 1 to 3, column 3 is
padding: 2 + 3 + 5 + 6 = 16. The bottom row gives 4 + 5 + 7 + 8 = 24 and
5 + 6 + 8 + 9 = 28. With the bias: 12.5, 16.5, 24.5, 28.5.

With dilation 2, a 2 by 2 kernel of ones spans 3 pixels and touches the four
corners: 1 + 3 + 7 + 9 = 20.

The plan checks every count before any value is read. Padding must be
smaller than the span, so every window overlaps the image. Each tensor holds
at most 2²⁰ values. The forward pass and the input gradient each do at most
2²⁴ multiply-adds.

<!-- source: psionic/lib/pixels.coax plan -->
```jai
// The product of four counts, or -1 once it passes the bound.
px_product :: (a:int,b:int,c:int,d:int,bound:int)->int {
    dims:=[4]int.[a,b,c,d];n:=1;
    for e:dims {if e>bound/n {return -1;}n*=e;}
    return n;
}
px_elements :: (a:int,b:int,c:int,d:int)->int {return px_product(a,b,c,d,PX_ELEMENTS_MAX);}
px_work :: (a:int,b:int,c:int,d:int)->int {return px_product(a,b,c,d,PX_WORK_MAX);}
px_conv_plan :: (c:PxConv)->PxConvPlan {
    extents:=[7]int.[c.batch,c.channels,c.height,c.width,c.filters,c.kernel_h,c.kernel_w];
    for e:extents {if e<1 {return PxConvPlan.{status=PxStatus.shape};}}
    for e:extents {if e>PX_EXTENT_MAX {return PxConvPlan.{status=PxStatus.limit};}}
    steps:=[5]int.[c.stride_h,c.stride_w,c.dilation_h,c.dilation_w,c.groups];
    for e:steps {if e<1 || e>PX_EXTENT_MAX {return PxConvPlan.{status=PxStatus.parameter};}}
    if c.channels%c.groups!=0 || c.filters%c.groups!=0 {return PxConvPlan.{status=PxStatus.parameter};}
    // The window a dilated kernel spans, and padding below it, so that every
    // window overlaps the image.
    span_h:=(c.kernel_h-1)*c.dilation_h+1;span_w:=(c.kernel_w-1)*c.dilation_w+1;
    if c.pad_h<0 || c.pad_w<0 || c.pad_h>=span_h || c.pad_w>=span_w {return PxConvPlan.{status=PxStatus.parameter};}
    if span_h>c.height+2*c.pad_h || span_w>c.width+2*c.pad_w {return PxConvPlan.{status=PxStatus.shape};}
    p:=PxConvPlan.{out_h=(c.height+2*c.pad_h-span_h)/c.stride_h+1,out_w=(c.width+2*c.pad_w-span_w)/c.stride_w+1};
    p.patch=c.channels/c.groups*c.kernel_h*c.kernel_w;
    p.x=px_elements(c.batch,c.channels,c.height,c.width);
    p.w=px_elements(c.filters,p.patch,1,1);
    p.y=px_elements(c.batch,c.filters,p.out_h,p.out_w);
    if p.x==-1 || p.w==-1 || p.y==-1 {return PxConvPlan.{status=PxStatus.limit};}
    // Forward multiply-adds; the input gradient visits every pixel's taps.
    p.work=px_work(c.batch*c.filters,p.out_h*p.out_w,p.patch,1);
    back:=px_work(c.batch*c.channels,c.height*c.width,c.filters/c.groups,c.kernel_h*c.kernel_w);
    if p.work==-1 || back==-1 {return PxConvPlan.{status=PxStatus.limit};}
    p.columns=c.batch*p.out_h*p.out_w;
    // im2col's columns for every group, chapter 4's staging for one group
    // and the product; -1 when that passes 2^20 values.
    p.scratch=px_elements(p.columns,c.channels*c.kernel_h*c.kernel_w+c.filters/c.groups+c.filters,1,1);
    return p;
}
```

### The direct sum

The direct sum computes each output with chapter 4's compensated accumulator
and rounds it to f32 once. It stages every value before writing any, so a sum
too large for f32 is refused with nothing written.

<!-- source: psionic/lib/pixels.coax direct -->
```jai
// Output (n, f, oh, ow) sums input (n, channel, oh*stride - pad + i*dilation,
// ow*stride - pad + j*dilation) times weight (f, channel within the group,
// i, j) over the channels of f's group and the kernel taps that land inside
// the image; taps in the padding read zero. No kernel flip: this is the
// cross-correlation every framework calls convolution.
px_conv_value :: (c:PxConv,p:PxConvPlan,x:[]f32,w:[]f32,bias:[]f32,i:int)->f64 {
    ow:=i%p.out_w;oh:=(i/p.out_w)%p.out_h;f:=(i/(p.out_w*p.out_h))%c.filters;n:=i/(p.out_w*p.out_h*c.filters);
    per:=c.channels/c.groups;first:=f/(c.filters/c.groups)*per;
    t:PsionicAccumulator;if bias.count>0 {t=psionic_ops_accumulate(t,f64(bias[f]));}
    for k:0..<per {for a:0..<c.kernel_h {
        row:=oh*c.stride_h-c.pad_h+a*c.dilation_h;if row<0 || row>=c.height {continue;}
        for b:0..<c.kernel_w {
            col:=ow*c.stride_w-c.pad_w+b*c.dilation_w;if col<0 || col>=c.width {continue;}
            pixel:=x[((n*c.channels+first+k)*c.height+row)*c.width+col];weight:=w[((f*per+k)*c.kernel_h+a)*c.kernel_w+b];
            t=psionic_ops_accumulate(t,f64(pixel)*f64(weight));
        }
    }}
    return psionic_ops_total(t);
}
px_conv_direct :: (c:PxConv,x:[]f32,w:[]f32,bias:[]f32,y:[]f32,scratch:[]f32,overlap:bool)->PxResult {
    p:=px_conv_admit(c,x,w,bias,y.count,scratch.count,false,overlap);if p.status!=PxStatus.ok {return PxResult.{status=p.status};}
    for i:0..<p.y {v:=px_conv_value(c,p,x,w,bias,i);if !psionic_ops_fits_f32(v) {return PxResult.{status=PxStatus.number};}scratch[i]=f32(v);}
    for i:0..<p.y {y[i]=scratch[i];}
    return PxResult.{written=p.y};
}
```

### Image to columns

The second path is how most libraries run a convolution. It copies, for each
output position, the pixels its window reads into one column. The
convolution then becomes a matrix product: each group's filters,
(filters/groups) rows of channels·kernel_h·kernel_w weights, times that
group's columns. Chapter 4's `psionic_ops_checked` does the product.

<!-- source: psionic/lib/pixels.coax im2col -->
```jai
// Image to columns: for group g, row (k, i, j) and column (n, oh, ow) hold the
// pixel that kernel tap (i, j) of channel k reads at that output, or zero in
// the padding. Each group's filters, a (filters/groups) by patch matrix, then
// multiply its columns with chapter 4's matrix product. Scratch holds the
// columns, chapter 4's staging, and the product, in that order.
px_conv_im2col :: (c:PxConv,x:[]f32,w:[]f32,bias:[]f32,work:[]f32,y:[]f32,overlap:bool)->PxResult {
    p:=px_conv_plan(c);if p.status!=PxStatus.ok {return PxResult.{status=p.status};}
    per:=c.channels/c.groups;each:=c.filters/c.groups;L:=p.columns;
    request:=PsionicRequest.{op=PsionicOp.matmul,rows=each,width=p.patch,columns=L};
    if p.scratch==-1 || psionic_ops_plan(request).status!=PsionicOpStatus.ok {return PxResult.{status=PxStatus.limit};}
    p=px_conv_admit(c,x,w,bias,y.count,work.count,true,overlap);if p.status!=PxStatus.ok {return PxResult.{status=p.status};}
    cols:=c.groups*p.patch*L;product:=cols+each*L;
    for r:0..<cols/L {
        g:=r/p.patch;k:=(r/(c.kernel_h*c.kernel_w))%per;a:=(r/c.kernel_w)%c.kernel_h;b:=r%c.kernel_w;
        for l:0..<L {
            ow:=l%p.out_w;oh:=(l/p.out_w)%p.out_h;n:=l/(p.out_w*p.out_h);
            row:=oh*c.stride_h-c.pad_h+a*c.dilation_h;col:=ow*c.stride_w-c.pad_w+b*c.dilation_w;v:f32=0;
            if row>=0 && row<c.height && col>=0 && col<c.width {v=x[((n*c.channels+g*per+k)*c.height+row)*c.width+col];}
            work[r*L+l]=v;
        }
    }
    idx:[1]int;mask:[1]u8;
    for g:0..<c.groups {
        r:=psionic_ops_checked(request,w[g*each*p.patch..<(g+1)*each*p.patch],work[g*p.patch*L..<(g+1)*p.patch*L],
            idx[0..<0],mask[0..<0],work[product+g*each*L..<product+(g+1)*each*L],work[cols..<product],false);
        if r.status!=PsionicOpStatus.ok {return PxResult.{status=PxStatus.number};}
    }
    // Add the bias in place, (filter, column), then copy to (n, filter, oh, ow).
    for i:0..<c.filters*L {
        v:=f64(work[product+i]);if bias.count>0 {v+=f64(bias[i/L]);}
        if !psionic_ops_fits_f32(v) {return PxResult.{status=PxStatus.number};}
        work[product+i]=f32(v);
    }
    area:=p.out_h*p.out_w;
    for i:0..<p.y {y[i]=work[product+(i/area)%c.filters*L+i/(area*c.filters)*area+i%area];}
    return PxResult.{written=p.y};
}
```

The two paths sum the same products in different orders and round in
different places: the direct sum adds the bias before its one rounding,
im2col rounds the product and then adds the bias. On the example's fixtures,
every value is a multiple of 1/256 and small, so every sum is exact and the
two paths must agree bit for bit. They do, on all 32 combinations of two
strides per axis, padding 0 and 1, dilation 1 and 2, and 1 and 2 groups. With
values that are not binary fractions (sevenths and ninths), 13 of 30 outputs
differ, by at most 1.2 × 10⁻⁷, one f32 step near 1.

On the setting chapter 5 supports (stride 1, no padding, no groups), the
checks also run chapter 5's `conv2d` operator and require each output to
equal the direct sum minus the bias.

## Gradients

Training needs the derivative of a loss L with respect to the input, the
weights and the bias, given dy = ∂L/∂y. Each comes from reading the forward
sum backwards:
- input pixel (n, ch, row, col) was multiplied by w[f, k, i, j] at every
  output with `oh·stride − pad + i·dilation = row` (and the same for
  columns), so dx gathers dy times w over those outputs;
- w[f, k, i, j] was multiplied by one pixel at every output of filter f, so
  dw sums pixel times dy over them;
- the bias was added once to every output of its filter, so db sums dy.

By hand: one 1 by 1 filter of weight 2 with bias 1 over a 2 by 2 image
1 2 / 3 4 gives y = 2x + 1. For dy = 1 0 / −1 0.5, dx = 2·dy = 2 0 / −2 1,
dw = 1·1 + 2·0 + 3·(−1) + 4·0.5 = 0 and db = 1 + 0 − 1 + 0.5 = 0.5. With
stride 2 on a 1 by 3 row, the middle pixel is never read, so its gradient is
0.

<!-- source: psionic/lib/pixels.coax backward -->
```jai
// The vector-Jacobian product of the convolution for an output gradient dy.
// Input pixel (n, ch, row, col) received weight (f, k, i, j) at every output
// (oh, ow) with oh*stride - pad + i*dilation = row; dx gathers those. dw sums
// pixel times dy over every output that used the weight, and db sums dy.
px_conv_dx :: (c:PxConv,p:PxConvPlan,w:[]f32,dy:[]f32,i:int)->f64 {
    col:=i%c.width;row:=(i/c.width)%c.height;ch:=(i/(c.width*c.height))%c.channels;n:=i/(c.width*c.height*c.channels);
    per:=c.channels/c.groups;each:=c.filters/c.groups;g:=ch/per;k:=ch%per;t:PsionicAccumulator;
    for e:0..<each {f:=g*each+e;for a:0..<c.kernel_h {
        u:=row+c.pad_h-a*c.dilation_h;if u<0 || u%c.stride_h!=0 || u/c.stride_h>=p.out_h {continue;}
        for b:0..<c.kernel_w {
            v:=col+c.pad_w-b*c.dilation_w;if v<0 || v%c.stride_w!=0 || v/c.stride_w>=p.out_w {continue;}
            t=psionic_ops_accumulate(t,f64(dy[((n*c.filters+f)*p.out_h+u/c.stride_h)*p.out_w+v/c.stride_w])*f64(w[((f*per+k)*c.kernel_h+a)*c.kernel_w+b]));
        }
    }}
    return psionic_ops_total(t);
}
px_conv_dw :: (c:PxConv,p:PxConvPlan,x:[]f32,dy:[]f32,i:int)->f64 {
    b:=i%c.kernel_w;a:=(i/c.kernel_w)%c.kernel_h;per:=c.channels/c.groups;k:=(i/(c.kernel_w*c.kernel_h))%per;f:=i/(c.kernel_w*c.kernel_h*per);
    ch:=f/(c.filters/c.groups)*per+k;t:PsionicAccumulator;
    for n:0..<c.batch {for oh:0..<p.out_h {
        row:=oh*c.stride_h-c.pad_h+a*c.dilation_h;if row<0 || row>=c.height {continue;}
        for ow:0..<p.out_w {
            col:=ow*c.stride_w-c.pad_w+b*c.dilation_w;if col<0 || col>=c.width {continue;}
            t=psionic_ops_accumulate(t,f64(x[((n*c.channels+ch)*c.height+row)*c.width+col])*f64(dy[((n*c.filters+f)*p.out_h+oh)*p.out_w+ow]));
        }
    }}
    return psionic_ops_total(t);
}
px_conv_db :: (c:PxConv,p:PxConvPlan,dy:[]f32,f:int)->f64 {
    t:PsionicAccumulator;area:=p.out_h*p.out_w;
    for n:0..<c.batch {for j:0..<area {t=psionic_ops_accumulate(t,f64(dy[(n*c.filters+f)*area+j]));}}
    return psionic_ops_total(t);
}
px_conv_backward :: (c:PxConv,x:[]f32,w:[]f32,dy:[]f32,dx:[]f32,dw:[]f32,db:[]f32,scratch:[]f32,overlap:bool)->PxResult {
    p:=px_conv_plan(c);if p.status!=PxStatus.ok {return PxResult.{status=p.status};}
    if x.count!=p.x || w.count!=p.w || dy.count!=p.y {return PxResult.{status=PxStatus.shape};}
    total:=p.x+p.w+c.filters;
    if dx.count<p.x || dw.count<p.w || db.count<c.filters || scratch.count<total {return PxResult.{status=PxStatus.capacity};}
    if overlap {return PxResult.{status=PxStatus.alias};}
    if !px_finite(x) || !px_finite(w) || !px_finite(dy) {return PxResult.{status=PxStatus.number};}
    for i:0..<total {
        v:=0.0;
        if i<p.x {v=px_conv_dx(c,p,w,dy,i);}
        else if i<p.x+p.w {v=px_conv_dw(c,p,x,dy,i-p.x);}
        else {v=px_conv_db(c,p,dy,i-p.x-p.w);}
        if !psionic_ops_fits_f32(v) {return PxResult.{status=PxStatus.number};}
        scratch[i]=f32(v);
    }
    for i:0..<p.x {dx[i]=scratch[i];}
    for i:0..<p.w {dw[i]=scratch[p.x+i];}
    for i:0..<c.filters {db[i]=scratch[p.x+p.w+i];}
    return PxResult.{written=total};
}
```

The input gradient solves for oh instead of looping over outputs, so it is
not the forward loop reversed. Central differences check it independently:
with L = Σ dy·y, the change in L when one input moves by ±h, divided by 2h,
is that input's derivative. A convolution is linear in its input, its
weights and its bias, so the central difference has no truncation error.
With every value a multiple of 1/256 and h = 1/4, every forward value is
exact in f32 and every loss exact in binary64. The checks then require
equality, not closeness, for every input, weight and bias on all 32
settings, and the example prints the count.

## Patches

A vision transformer cuts an image into P by P squares, flattens each square
into one row of channels·P·P features, and multiplies the rows by a weight
matrix. The result is one token per square.

Take a 1-channel 2 by 4 image with rows 1 2 3 4 and 5 6 7 8, and P = 2. The
left square is 1 2 / 5 6 and the right square is 3 4 / 7 8, so the rows are
1 2 5 6 and 3 4 7 8. With two output features, one weight row 1 0 0 0
picking the top-left pixel and one 1 1 1 1 summing the square, plus biases 0
and 0.5, the tokens are (1, 14.5) and (3, 22.5).

ViT-B/16 at 224 pixels has 196 tokens of 3 · 16 · 16 = 768 features and
projects them to 768: 196 · 768 · 768 = 115,605,504 multiply-adds. Chapter 4's
product takes at most 2²⁰ = 1,048,576 at a time, so the projection runs in
tiles. Every tile multiplies all of the weights by as many tokens as keep it
within 2²⁰ − 1 multiply-adds, at least one: 768 · 768 = 589,824 weights fit
once, so each of the 196 tiles holds one token. Each output's whole inner
product happens inside one call, so tiling changes the size admitted and
nothing else.

The tiles write their outputs directly, so nothing may fail once the first
has written. Admission bounds every sum first: features times the largest
pixel times the largest weight, plus the largest bias, must be at most 2¹²⁷,
half of f32's range. Inputs that could pass it are refused before anything is
written, even if their actual sums would fit.

<!-- source: psionic/lib/pixels.coax patch -->
```jai
// Each tensor holds at most 2^20 values and the projection does at most 2^28
// multiply-adds. Chapter 4's product takes at most 2^20 multiply-adds at a
// time, so the plan cuts the tokens into tiles: as many tokens as keep one
// tile within 2^20 - 1 multiply-adds beside all of the weights.
px_patch_plan :: (q:PxPatch)->PxPatchPlan {
    extents:=[6]int.[q.batch,q.channels,q.height,q.width,q.patch,q.dim];
    for e:extents {if e<1 {return PxPatchPlan.{status=PxStatus.shape};}}
    for e:extents {if e>PX_EXTENT_MAX {return PxPatchPlan.{status=PxStatus.limit};}}
    if q.height%q.patch!=0 || q.width%q.patch!=0 {return PxPatchPlan.{status=PxStatus.shape};}
    p:=PxPatchPlan.{grid_h=q.height/q.patch,grid_w=q.width/q.patch};
    p.tokens=p.grid_h*p.grid_w;p.features=q.channels*q.patch*q.patch;
    p.x=px_elements(q.batch,q.channels,q.height,q.width);p.w=px_elements(q.dim,p.features,1,1);p.out=px_elements(q.batch,p.tokens,q.dim,1);
    if p.x==-1 || p.w==-1 || p.out==-1 {return PxPatchPlan.{status=PxStatus.limit};}
    total:=q.batch*p.tokens;p.work=px_patch_work(total,p.features,q.dim);
    if p.work==-1 {return PxPatchPlan.{status=PxStatus.limit};}
    // At least one token per tile: 2^20 weights fill chapter 4 exactly.
    rows:=PX_TILE_WORK/p.w;if rows==0 {rows=1;}p.rows=px_fewer(rows,total);p.scratch=p.rows*(p.features+2*q.dim);
    return p;
}
// The smaller of two counts.
px_patch_work :: (a:int,b:int,c:int)->int {return px_product(a,b,c,1,PX_PATCH_WORK_MAX);}
px_fewer :: (a:int,b:int)->int {return (a+b-tv_magnitude(a-b))/2;}
// Where feature k of token r sits in the image: image r / tokens, grid cell
// r % tokens, and within its square (channel, row, column).
px_patch_pixel :: (q:PxPatch,p:PxPatchPlan,r:int,k:int)->int {
    n:=r/p.tokens;gy:=(r%p.tokens)/p.grid_w;gx:=r%p.grid_w;ch:=k/(q.patch*q.patch);dy:=(k/q.patch)%q.patch;dx:=k%q.patch;
    return ((n*q.channels+ch)*q.height+gy*q.patch+dy)*q.width+gx*q.patch+dx;
}
// Cut each image into patch by patch squares, left to right and top to
// bottom; each square becomes one row of features in (channel, row, column)
// order, the order of the convolution's weights.
px_patchify :: (q:PxPatch,x:[]f32,tokens:[]f32)->int {
    p:=px_patch_plan(q);if p.status!=PxStatus.ok || x.count!=p.x || tokens.count<p.x {return 0;}
    for i:0..<p.x {tokens[i]=x[px_patch_pixel(q,p,i/p.features,i%p.features)];}
    return p.x;
}
// The weights times the tokens, plus the bias: the first layer of a vision
// transformer. Chapter 4 multiplies one tile of tokens at a time, each
// output's whole inner product in one call, so the tiles change nothing but
// the size admitted. Admission bounds every sum first: features times the
// largest pixel times the largest weight, plus the largest bias, at most
// 2^127. No tile can then fail, and each writes its outputs directly.
// Scratch holds one tile's tokens, its product and chapter 4's staging.
px_patch_embed :: (q:PxPatch,x:[]f32,w:[]f32,bias:[]f32,work:[]f32,out:[]f32,overlap:bool)->PxResult {
    p:=px_patch_plan(q);if p.status!=PxStatus.ok {return PxResult.{status=p.status};}
    if x.count!=p.x || w.count!=p.w || bias.count!=q.dim {return PxResult.{status=PxStatus.shape};}
    if work.count<p.scratch || out.count<p.out {return PxResult.{status=PxStatus.capacity};}
    if overlap {return PxResult.{status=PxStatus.alias};}
    if !px_finite(x) || !px_finite(w) || !px_finite(bias) {return PxResult.{status=PxStatus.number};}
    if !(f64(p.features)*px_largest(x)*px_largest(w)+px_largest(bias)<=PX_SUM_MAX) {return PxResult.{status=PxStatus.number};}
    return px_patch_tiles(q,p,x,w,bias,work,out);
}
px_largest :: (x:[]f32)->f64 {m:=0.0;for v:x {m=px_most(m,abs(f64(v)));}return m;}
// The product for an admitted plan, p.rows tokens at a time.
px_patch_tiles :: (q:PxPatch,p:PxPatchPlan,x:[]f32,w:[]f32,bias:[]f32,work:[]f32,out:[]f32)->PxResult {
    total:=q.batch*p.tokens;idx:[1]int;mask:[1]u8;
    for tile:0..<(total+p.rows-1)/p.rows {
        first:=tile*p.rows;columns:=px_fewer(total-first,p.rows);
        // The tile's tokens as columns: feature k of token first + j at k * columns + j.
        for i:0..<p.features*columns {work[i]=x[px_patch_pixel(q,p,first+i%columns,i/columns)];}
        tokens:=p.features*columns;each:=q.dim*columns;
        r:=psionic_ops_checked(PsionicRequest.{op=PsionicOp.matmul,rows=q.dim,width=p.features,columns=columns},w,work[0..<tokens],idx[0..<0],mask[0..<0],
            work[tokens..<tokens+each],work[tokens+each..<tokens+2*each],false);
        #assert r.status==PsionicOpStatus.ok;
        // Row d, column j of the product is token first + j, feature d.
        for i:0..<each {out[(first+i%columns)*q.dim+i/columns]=f32(f64(work[tokens+i])+f64(bias[i/columns]));}
    }
    return PxResult.{written=p.out};
}
// One projected value as a direct sum, bias included, in binary64 with
// chapter 4's compensated accumulator. Weight (d, k) is at d * features + k.
px_patch_value :: (q:PxPatch,p:PxPatchPlan,x:[]f32,w:[]f32,bias:[]f32,i:int)->f64 {
    d:=i%q.dim;r:=i/q.dim;s:PsionicAccumulator;s=psionic_ops_accumulate(s,f64(bias[d]));
    for k:0..<p.features {s=psionic_ops_accumulate(s,f64(x[px_patch_pixel(q,p,r,k)])*f64(w[d*p.features+k]));}
    return psionic_ops_total(s);
}
```

Cutting into squares and multiplying is a convolution whose kernel and
stride are both P. The weights, stored as (dim, channels, P, P), are the
convolution's weights. The example runs the same image as a stride-2
convolution. It prints the same four numbers in the convolution's order,
filter before position. The checks require equality on every element for
patch sizes 1 to 3 and batches of 1 and 2, and equality with the direct sum
`px_patch_value` for tiles of every size from one token to all six.

## Lower into chapter 38's tensor IR

Chapter 38's IR has one convolution node: valid, stride one, one group. A
convolution with any other setting becomes that node by rearranging its
inputs, without new arithmetic.

Tap i of a kernel dilated by d reads row `oh · stride + i · d` of the padded
image. Write `i · d = q · stride + r` with r below the stride. Then the tap
reads row `oh + q` of **phase** r: the plane of every stride-th padded row,
starting at row r. Each phase, in each direction, becomes one input channel
of a stride-one convolution, and tap i becomes weight q of channel r. Weights
where no tap lands, or where a filter's group does not reach a channel, are
zero. The bias is chapter 38's broadcast add.

Take the 3 by 3 box sum again: stride 2, padding 1, a 3 by 3 kernel. Taps 0,
1 and 2 have (q, r) = (0, 0), (0, 1) and (1, 0), so each direction has two
phases and two weights, and the lowered convolution has 4 channels and a 2
by 2 kernel over 3 by 3 phase planes. Phase (0, 0) holds padded rows and
columns 0, 2 and 4, which are image rows and columns −1, 1 and 3: zero
everywhere but the 5 in its centre. Phase (1, 1) holds image rows and
columns 0 and 2: 1 3 / 7 9 and zeros. Its program is five nodes: the phases,
the spread weights and the bias as inputs, the convolution, and the add. It
prints 12.5, 16.5, 24.5 and 28.5, on chapter 5's reference arithmetic and on
chapter 38's planned kernels.

<!-- source: psionic/lib/pixels-ir.coax ir-types -->
```jai
// The chapter 38 program for one convolution, and the counts of its inputs.
PxLowered :: struct {status:PxStatus;program:TiProgram;plan:PxConvPlan;reuse:bool;taps_h:int;taps_w:int;channels:int;height:int;width:int;
    x:int;w:int;input:int;workspace:int;stage:int;flags:int;}
```

<!-- source: psionic/lib/pixels-ir.coax lower -->
```jai
// Tap i of a kernel dilated by d reads row oh * stride + i * d of the padded
// image. Write i * d = q * stride + r: the tap reads row oh + q of phase r,
// the plane of every stride-th padded row starting at r. Each phase becomes
// one input channel, and tap i becomes weight q of that channel: a valid,
// stride-one convolution with taps_h = (kernel_h - 1) * d / stride + 1 rows.
px_lower_conv :: (c:PxConv,bias:bool,reuse:bool)->PxLowered {
    p:=px_conv_plan(c);if p.status!=PxStatus.ok {return PxLowered.{status=p.status};}
    l:=PxLowered.{plan=p,reuse=reuse,taps_h=(c.kernel_h-1)*c.dilation_h/c.stride_h+1,taps_w=(c.kernel_w-1)*c.dilation_w/c.stride_w+1};
    l.channels=c.channels*c.stride_h*c.stride_w;l.height=p.out_h+l.taps_h-1;l.width=p.out_w+l.taps_w-1;
    // Chapter 38 bounds these counts when it compiles the program.
    l.x=c.batch*l.channels*l.height*l.width;l.w=c.filters*l.channels*l.taps_h*l.taps_w;
    // Inputs: phases, spread weights and, with a bias, the bias as (filters, 1, 1).
    l.program.nodes[0]=ti_input(px_ir_shape(c.batch,l.channels,l.height,l.width),0);
    l.program.nodes[1]=ti_input(px_ir_shape(c.filters,l.channels,l.taps_h,l.taps_w),l.x);
    l.program.nodes[2]=TiNode.{op=TiOp.conv2d,a=0,b=1};l.program.count=3;l.program.output=2;l.input=l.x+l.w;
    if bias {
        l.program.nodes[2]=ti_input(TvShape.{rank=3,dims=[4]int.[c.filters,1,1,0]},l.input);
        l.program.nodes[3]=TiNode.{op=TiOp.conv2d,a=0,b=1};l.program.nodes[4]=TiNode.{op=TiOp.add,a=3,b=2};
        l.program.count=5;l.program.output=4;l.input+=c.filters;
    }
    plan:=ti_compile(l.program,TiTarget.{},reuse);
    if plan.status!=TiStatus.ok {return PxLowered.{status=px_ir_status(plan.status)};}
    l.workspace=plan.workspace;l.stage=plan.output_elements;l.flags=plan.flag_elements;
    return l;
}
// Pack the program's inputs: the padded phases, the spread weights, the bias.
px_lower_inputs :: (c:PxConv,l:PxLowered,x:[]f32,w:[]f32,bias:[]f32,input:[]f32)->PxResult {
    if input.count<l.input {return PxResult.{status=PxStatus.capacity};}
    for i:0..<l.x {
        v:=i%l.width;u:=(i/l.width)%l.height;phase:=(i/(l.width*l.height))%l.channels;n:=i/(l.width*l.height*l.channels);
        ch:=phase/(c.stride_h*c.stride_w);row:=u*c.stride_h+(phase/c.stride_w)%c.stride_h-c.pad_h;col:=v*c.stride_w+phase%c.stride_w-c.pad_w;
        value:f32=0;if row>=0 && row<c.height && col>=0 && col<c.width {value=x[((n*c.channels+ch)*c.height+row)*c.width+col];}
        input[i]=value;
    }
    // Weight (f, phase, q_h, q_w) is tap (row, column) = ((q * stride + r) /
    // dilation) when that divides exactly, lies inside the kernel, and the
    // phase's channel is in f's group; zero otherwise.
    per:=c.channels/c.groups;each:=c.filters/c.groups;
    for i:0..<l.w {
        qw:=i%l.taps_w;qh:=(i/l.taps_w)%l.taps_h;phase:=(i/(l.taps_w*l.taps_h))%l.channels;f:=i/(l.taps_w*l.taps_h*l.channels);
        k:=phase/(c.stride_h*c.stride_w)-f/each*per;down:=qh*c.stride_h+(phase/c.stride_w)%c.stride_h;across:=qw*c.stride_w+phase%c.stride_w;
        value:f32=0;
        if k>=0 && k<per && down%c.dilation_h==0 && down/c.dilation_h<c.kernel_h && across%c.dilation_w==0 && across/c.dilation_w<c.kernel_w {
            value=w[((f*per+k)*c.kernel_h+down/c.dilation_h)*c.kernel_w+across/c.dilation_w];
        }
        input[l.x+i]=value;
    }
    for i:0..<l.input-l.x-l.w {input[l.x+l.w+i]=bias[i];}
    return PxResult.{written=l.input};
}
```

A patch projection lowers as the convolution with kernel and stride P plus
one view node. The view reads the (batch, dim, grid) result as (batch,
token, dim), and chapter 38's publication copies it out in that order.

<!-- source: psionic/lib/pixels-ir.coax ir-run -->
```jai
// Lower, pack and run on chapter 38's CPU path: reuse selects its planned
// kernels, otherwise chapter 5's reference arithmetic. Scratch holds the
// program's inputs, its workspace and its staged output.
px_conv_ir :: (c:PxConv,x:[]f32,w:[]f32,bias:[]f32,reuse:bool,scratch:[]f32,flags:[]u8,y:[]f32)->PxResult {
    l:=px_lower_conv(c,bias.count>0,reuse);if l.status!=PxStatus.ok {return PxResult.{status=l.status};}
    return px_ir_run(c,l,x,w,bias,scratch,flags,y);
}
px_ir_run :: (c:PxConv,l:PxLowered,x:[]f32,w:[]f32,bias:[]f32,scratch:[]f32,flags:[]u8,y:[]f32)->PxResult {
    if x.count!=l.plan.x || w.count!=l.plan.w || (bias.count!=0 && bias.count!=c.filters) {return PxResult.{status=PxStatus.shape};}
    if scratch.count<l.input+l.workspace+l.stage || flags.count<l.flags || y.count<l.stage {return PxResult.{status=PxStatus.capacity};}
    #assert px_lower_inputs(c,l,x,w,bias,scratch[0..<l.input]).written==l.input;
    work:=l.input+l.workspace;
    r:=ti_cpu_checked(l.program,l.reuse,scratch[0..<l.input],scratch[l.input..<work],scratch[work..<work+l.stage],y,flags[0..<l.flags],false);
    if r.status!=TiStatus.ok {return PxResult.{status=px_ir_status(r.status)};}
    return PxResult.{written=r.written};
}
// A patch projection is the convolution with kernel and stride both the
// patch size; one more node views its (batch, dim, grid) result as (batch,
// token, dim), and chapter 38's publication copies it out in that order.
px_patch_ir :: (q:PxPatch,x:[]f32,w:[]f32,bias:[]f32,reuse:bool,scratch:[]f32,flags:[]u8,out:[]f32)->PxResult {
    p:=px_patch_plan(q);if p.status!=PxStatus.ok {return PxResult.{status=p.status};}
    if bias.count!=q.dim {return PxResult.{status=PxStatus.shape};}
    c:=PxConv.{batch=q.batch,channels=q.channels,height=q.height,width=q.width,filters=q.dim,kernel_h=q.patch,kernel_w=q.patch,stride_h=q.patch,stride_w=q.patch};
    l:=px_lower_conv(c,true,reuse);if l.status!=PxStatus.ok {return PxResult.{status=l.status};}
    l.program.nodes[5]=TiNode.{op=TiOp.view,a=4,shape=TvShape.{rank=3,dims=[4]int.[q.batch,p.tokens,q.dim,0]},strides=[4]int.[q.dim*p.tokens,1,p.tokens,0]};
    // A view adds no storage, command or flag: the counts stay the convolution's.
    l.program.count=6;l.program.output=5;
    return px_ir_run(c,l,x,w,bias,scratch,flags,out);
}
```

Chapter 38 bounds what the lowering can build. Chapter 5's 2²⁰ work limit
counts the zero weights too, and padding enlarges the input: a padded 1,024
by 1,024 image is 1,026 by 1,026, past chapter 38's 2²⁰ values. Each is
refused as a limit.

## Position tables at a new size

A vision transformer learns one position vector per grid cell. At 224 by 224
pixels with 16-pixel patches the grid is 14 by 14. Run the same model on a
448-pixel image and the grid is 28 by 28. The usual fix resizes the learned
table as if it were an image with one channel per feature. `px_positions` does
that with the bilinear rule above, reading the table in its natural
(row, column, feature) order.

<!-- source: psionic/lib/pixels.coax positions -->
```jai
// A learned position table, one row of dim values per grid cell (row,
// column, feature), resized to a new grid by the bilinear rule above.
px_positions :: (grid_h:int,grid_w:int,dim:int,table:[]f32,new_h:int,new_w:int,out:[]f32,overlap:bool)->PxResult {
    if grid_h<1 || grid_w<1 || dim<1 {return PxResult.{status=PxStatus.shape};}
    if grid_h>PX_SIDE_MAX || grid_w>PX_SIDE_MAX || dim>PX_EXTENT_MAX {return PxResult.{status=PxStatus.limit};}
    if new_h<1 || new_w<1 {return PxResult.{status=PxStatus.parameter};}
    if new_h>PX_SIDE_MAX || new_w>PX_SIDE_MAX {return PxResult.{status=PxStatus.limit};}
    n:=px_elements(grid_h,grid_w,dim,1);m:=px_elements(new_h,new_w,dim,1);
    if n==-1 || m==-1 {return PxResult.{status=PxStatus.limit};}
    if table.count!=n {return PxResult.{status=PxStatus.shape};}
    if out.count<m {return PxResult.{status=PxStatus.capacity};}
    if overlap {return PxResult.{status=PxStatus.alias};}
    if !px_finite(table) {return PxResult.{status=PxStatus.number};}
    for i:0..<m {
        d:=i%dim;t:=i/dim;
        out[i]=f32(px_sample(table,d,grid_w*dim,dim,grid_h,grid_w,new_h,new_w,PxFilter.bilinear,t/new_w,t%new_w));
    }
    return PxResult.{written=m};
}
```

A 2 by 2 table holding 0 1 / 1 0 stretched to 4 by 4 gives the same numbers
as the red channel above before normalization. The checks compare
`px_positions` with chapter 5's `bilinear2d` run through a chapter 3 view.
The view swaps the table's axes to put the feature axis first without copying
anything, and the outputs must be identical.

## A photo, ViT-B/16, and a compiled convolution

The [second example](../examples/pixels-vit.coax) runs the rules above on the
2 by 2 image and the halved row, prepares a 320 by 240 photo the way
ImageNet evaluation does, with PIL's antialiased bicubic filter, and projects
the crop with ViT-B/16's shapes. It then compiles a convolution with two
groups, stride (2, 1), padding 1 and dilation (2, 3) through chapter 38.

<!-- source: psionic/examples/pixels-vit.coax main -->
```jai
main :: () {
    // The 2 by 2 image to 4 by 4, red row 1, under three more rules.
    pixels:=px_tiny();work:[12]f32;out:[48]f32;image:=PxImage.{width=2,height=2};
    cubic:=px_half();cubic.filter=PxFilter.bicubic;r:=px_preprocess(image,pixels[..],cubic,work[..],out[..],false);
    show("red row 1, bicubic:",out[4..<8]);
    aligned:=px_half();aligned.corners=true;r=px_preprocess(image,pixels[..],aligned,work[..],out[..],false);
    show("red row 1, bilinear, corners aligned:",out[4..<8]);
    // A red row 0 0 255 255 halved, without and with antialiasing.
    row:=[12]u8.[0,0,0, 0,0,0, 255,0,0, 255,0,0];four:=PxImage.{width=4,height=1};
    plain:=PxProfile.{scale=1.0/255,mean=[3]f64.[0,0,0],std=[3]f64.[1,1,1],height=1,width=2,filter=PxFilter.bilinear};
    r=px_preprocess(four,row[..],plain,work[..],out[0..<6],false);show("4 to 2, bilinear:",out[0..<2]);
    plain.antialias=true;r=px_preprocess(four,row[..],plain,work[..],out[0..<6],false);show("4 to 2, antialiased:",out[0..<2]);

    // A 320 by 240 photo: shorter side to 256, a centred 224 crop, PIL's bicubic,
    // then ViT-B/16's projection of the crop.
    x:[150528]f32;vit_prepare(x[..]);vit_project(x[..]);

    // Two groups, stride (2, 1), padding 1, dilation (2, 3), through chapter 38.
    c:=PxConv.{batch=2,channels=4,height=7,width=6,filters=4,kernel_h=3,kernel_w=2,stride_h=2,pad_h=1,pad_w=1,dilation_h=2,dilation_w=3,groups=2};
    l:=px_lower_conv(c,true,true);cp:=px_conv_plan(c);
    print("lowered: % nodes, % phase channels of %x%, %x% taps\n",l.program.count,l.channels,l.height,l.width,l.taps_h,l.taps_w);
    cx:[336]f32;cw:[48]f32;cb:[4]f32;px_fill(cx[..],5);px_fill(cw[0..<cp.w],3);px_fill(cb[..],7);
    direct:[120]f32;ds:[120]f32;r=px_conv_direct(c,cx[..],cw[0..<cp.w],cb[..],direct[0..<cp.y],ds[0..<cp.y],false);
    scratch:[2000]f32;flags:[400]u8;
    for reuse:0..<2 {
        y:[120]f32;r=px_conv_ir(c,cx[..],cw[0..<cp.w],cb[..],reuse==1,scratch[..],flags[..],y[0..<cp.y]);
        same:=0;for i:0..<cp.y {if y[i]==direct[i] {same+=1;}}
        names:=[2]string.["chapter 5's reference arithmetic","chapter 38's planned kernels"];
        print("  %: %, % of % outputs equal the direct sum\n",names[reuse],r.status,same,cp.y);
    }
}
```

<!-- source: psionic/examples/pixels-vit.coax photo -->
```jai
vit_prepare :: (x:[]f32) {
    photo:=PxImage.{width=320,height=240};bytes:[230400]u8;
    for i:0..<230400 {bytes[i]=cast(u8)(((i/3)%320*7+i/960*3+i%3*85)%256);}
    net:=px_imagenet();net.shorter=256;net.filter=PxFilter.bicubic;net.antialias=true;
    fit:=px_fit(240,320,net);print("320x240 resized to %x%, cropped at row % column %\n",fit.width,fit.height,fit.top,fit.left);
    work:[230400]f32;r:=px_preprocess(photo,bytes[..],net,work[..],x,false);
    worst:=0.0;
    for s:0..<150 {i:=s*1003;d:=abs(f64(x[i])-px_reference(photo,bytes[..],net,i/50176,(i/224)%224,i%224));if d>worst {worst=d;}}
    print("prepared: %, % values; 150 against the reference, largest difference %\n",r.status,r.written,worst);
}
// 196 tokens of 768 features to 768, against the direct sum of every output.
vit_project :: (x:[]f32) {
    q:=PxPatch.{channels=3,height=224,width=224,patch=16,dim=768};p:=px_patch_plan(q);
    w:[589824]f32;for i:0..<589824 {w[i]=f32((i*37+11)%101-50)/1024;}
    bias:[768]f32;for i:0..<768 {bias[i]=f32(i%9-4)/32;}
    work:[2304]f32;tokens:[150528]f32;r:=px_patch_embed(q,x,w[..],bias[..],work[0..<p.scratch],tokens[..],false);
    differ:=0;gap:=0.0;
    for i:0..<p.out {v:=px_patch_value(q,p,x,w[..],bias[..],i);if f32(v)!=tokens[i] {differ+=1;}e:=abs(f64(tokens[i])-v);if e>gap {gap=e;}}
    print("ViT-B/16 projection: %, % multiply-adds, % tiles of % token\n",r.status,p.work,p.tokens/p.rows,p.rows);
    print("  % of % values differ from the direct sum, by at most %\n",differ,p.out,gap);
}
```

```text
red row 1, bicubic: -0.66223145 -0.29907227 0.29907227 0.66223145
red row 1, bilinear, corners aligned: -0.3333333 -0.111111104 0.111111164 0.33333337
4 to 2, bilinear: 0.0 1.0
4 to 2, antialiased: 0.14285715 0.85714287
320x240 resized to 341x256, cropped at row 16 column 58
prepared: ok, 150528 values; 150 against the reference, largest difference 2.440068355147318e-07
ViT-B/16 projection: ok, 115605504 multiply-adds, 196 tiles of 1 token
  30408 of 150528 values differ from the direct sum, by at most 8.412189345108345e-08
lowered: 5 nodes, 8 phase channels of 5x8, 3x4 taps
  chapter 5's reference arithmetic: ok, 120 of 120 outputs equal the direct sum
  chapter 38's planned kernels: ok, 120 of 120 outputs equal the direct sum
```

It runs in about 45 seconds in debug and a few in release, nearly all of it
in the two 115,605,504-multiply-add sums. The 30,408 outputs that differ do
so by one rounding: the tiled product rounds its inner sum to f32 before the
bias is added, the direct sum rounds once. The photo, the weights and the
crop each live in their own procedure because a debug build has about 4 MB
of stack.

## Refusals

Every malformed input is refused before anything is written, with a status
that says what was wrong. The [refusal example](../examples/pixels-refuse.coax)
feeds one of each:

<!-- source: psionic/examples/pixels-refuse.coax refuse -->
```jai
main :: () {
    pixels:=px_tiny();work:[12]f32;out:[48]f32;for i:0..<48 {out[i]=-7;}p:=px_half();
    four:=PxImage.{width=2,height=2,channels=4};print("four channels: %\n",px_preprocess(four,pixels[..],p,work[..],out[..],false).status);
    deep:=PxImage.{width=2,height=2,depth=16};print("16-bit samples: %\n",px_preprocess(deep,pixels[..],p,work[..],out[..],false).status);
    flat:=PxImage.{width=0,height=2};print("width 0: %\n",px_preprocess(flat,pixels[..],p,work[..],out[..],false).status);
    wide:=PxImage.{width=513,height=2};print("width 513: %\n",px_preprocess(wide,pixels[..],p,work[..],out[..],false).status);
    image:=PxImage.{width=2,height=2};
    print("11 bytes for a 2x2 RGB image: %\n",px_preprocess(image,pixels[0..<11],p,work[..],out[..],false).status);
    zero:=p;zero.std[1]=0;print("spread 0 in green: %\n",px_preprocess(image,pixels[..],zero,work[..],out[..],false).status);
    print("47 output values for 48: %\n",px_preprocess(image,pixels[..],p,work[..],out[0..<47],false).status);
    soft:=p;soft.filter=PxFilter.nearest;soft.antialias=true;print("antialiased nearest: %\n",px_preprocess(image,pixels[..],soft,work[..],out[..],false).status);
    soft=p;soft.antialias=true;soft.corners=true;print("antialiasing with corners aligned: %\n",px_preprocess(image,pixels[..],soft,work[..],out[..],false).status);
    crop:=p;crop.shorter=2;crop.height=3;print("a 3-row crop of a 2-row resize: %\n",px_preprocess(image,pixels[..],crop,work[..],out[..],false).status);
    crop=p;crop.shorter=513;print("shorter side 513: %\n",px_preprocess(image,pixels[..],crop,work[..],out[..],false).status);
    untouched:=0;for v:out {if v==-7 {untouched+=1;}}print("output values still -7: % of 48\n",untouched);

    x:=[9]f32.[1,2,3,4,5,6,7,8,9];w:=[9]f32.[1,1,1,1,1,1,1,1,1];none:[1]f32;y:=[4]f32.[-7,-7,-7,-7];s:[4]f32;
    c:=PxConv.{channels=1,height=3,width=3,filters=1,kernel_h=3,kernel_w=3};
    bad:=c;bad.pad_h=3;print("padding 3 on a 3-wide window: %\n",px_conv_direct(bad,x[..],w[..],none[0..<0],y[..],s[..],false).status);
    bad=c;bad.stride_w=0;print("stride 0: %\n",px_conv_direct(bad,x[..],w[..],none[0..<0],y[..],s[..],false).status);
    bad=c;bad.groups=2;print("2 groups of 1 channel: %\n",px_conv_direct(bad,x[..],w[..],none[0..<0],y[..],s[..],false).status);
    bad=c;bad.dilation_h=2;print("dilation 2 spans 5 rows of 3: %\n",px_conv_direct(bad,x[..],w[..],none[0..<0],y[..],s[..],false).status);
    print("8 weights for 9: %\n",px_conv_direct(c,x[..],w[0..<8],none[0..<0],y[..],s[..],false).status);
    x[4]=0.0/0.0;print("a NaN pixel: %\n",px_conv_direct(c,x[..],w[..],none[0..<0],y[..],s[..],false).status);x[4]=5;
    x[4]=f32_from_bits(0x7f7fffff);x[5]=x[4];print("a sum past the largest f32: %\n",px_conv_direct(c,x[..],w[..],none[0..<0],y[..],s[..],false).status);
    print("outputs still -7: % % % %\n",y[0],y[1],y[2],y[3]);
    huge:=PxConv.{channels=520,height=1,width=1024,filters=2,kernel_h=1,kernel_w=1};
    print("im2col with 2^20 + 16,384 multiply-adds in one product: %\n",px_conv_im2col(huge,x[..],w[..],none[0..<0],s[..],y[..],false).status);
    big:=PxConv.{channels=2,height=1024,width=1024,filters=1,kernel_h=1,kernel_w=1};print("2^21 input values: %\n",px_conv_plan(big).status);

    q:=PxPatch.{channels=3,height=8,width=10,patch=4,dim=5};print("patch 4 on width 10: %\n",px_patch_plan(q).status);
    q=PxPatch.{batch=257,channels=1,height=32,width=32,patch=32,dim=1024};print("a projection of 2^28 + 2^20 multiply-adds: %\n",px_patch_plan(q).status);
    q=PxPatch.{channels=1,height=2,width=4,patch=2,dim=2};img:=[8]f32.[1,2,3,4,5,6,7,8];pw:=[8]f32.[1,0,0,0,1,1,1,1];pb:=[2]f32.[0,0.5];pwork:[16]f32;
    embed:=[4]f32.[-7,-7,-7,-7];img[0]=f32_from_bits(0x7f000000);
    print("a pixel of 2^127 in a projection: %, outputs still % %\n",px_patch_embed(q,img[..],pw[..],pb[..],pwork[..],embed[..],false).status,embed[0],embed[3]);
    pad:=PxConv.{channels=1,height=1024,width=1024,filters=1,kernel_h=3,kernel_w=3,pad_h=1,pad_w=1};
    print("lowering a padded 1024x1024 image to chapter 38: %\n",px_lower_conv(pad,false,false).status);
    table:=[4]f32.[0,1,1,0];grid:[16]f32;
    print("a position table resized to 0 rows: %\n",px_positions(2,2,1,table[..],0,4,grid[..],false).status);
    print("3 table values for a 2x2 grid: %\n",px_positions(2,2,1,table[0..<3],4,4,grid[..],false).status);
}
```

```text
four channels: channels
16-bit samples: depth
width 0: shape
width 513: limit
11 bytes for a 2x2 RGB image: shape
spread 0 in green: parameter
47 output values for 48: capacity
antialiased nearest: parameter
antialiasing with corners aligned: parameter
a 3-row crop of a 2-row resize: shape
shorter side 513: limit
output values still -7: 48 of 48
padding 3 on a 3-wide window: parameter
stride 0: parameter
2 groups of 1 channel: parameter
dilation 2 spans 5 rows of 3: shape
8 weights for 9: shape
a NaN pixel: number
a sum past the largest f32: number
outputs still -7: -7.0 -7.0 -7.0 -7.0
im2col with 2^20 + 16,384 multiply-adds in one product: limit
2^21 input values: limit
patch 4 on width 10: shape
a projection of 2^28 + 2^20 multiply-adds: limit
a pixel of 2^127 in a projection: number, outputs still -7.0 -7.0
lowering a padded 1024x1024 image to chapter 38: limit
a position table resized to 0 rows: parameter
3 table values for a 2x2 grid: shape
```

## Supported profile

`psionic-pixels-v1`, bounded CPU:
- 8-bit, three-channel images up to 512 by 512, in RGB or BGR order;
  profiles with scale in (0, 1], means within [−256, 256] and spreads within
  [2⁻¹⁰, 256], output sides up to 512, shorter sides up to 512 with resized
  sides up to 4,096;
- nearest and half-pixel bilinear resizing exactly as chapter 5's operators;
  nearest, bilinear and bicubic (a = −0.75) with aligned corners; bicubic at
  half-pixel centres; antialiased bilinear and bicubic (a = −0.5) as PIL
  computes them, kept in f32 rather than rounded to bytes after each pass as
  PIL does for 8-bit images;
- convolution extents, strides and dilations up to 4,096, padding below the
  kernel's span, groups that divide both channel counts;
- at most 2²⁰ values per tensor and 2²⁴ multiply-adds forward and for the
  input gradient; im2col's scratch at most 2²⁰ values, and each group's
  product at most 2²⁰ multiply-adds (chapter 4's bound);
- patch projections of at most 2²⁸ multiply-adds in tiles of at most
  2²⁰ − 1, with every sum bounded by 2¹²⁷;
- lowered convolutions within chapter 38's and chapter 5's bounds, on the
  CPU;
- f32 storage, binary64 sums with chapter 4's compensated accumulator, one
  rounding to f32 per stored value.

## How it is checked

- **Coax checks:** 26 checks in `pixels.coax`, 96 counting the units it
  loads, and 4 in `pixels-ir.coax`, 160 counting its units, in debug and
  release:
  - the hand-worked resizes and weights: half-pixel, aligned corners,
    bicubic and antialiased; the fit of a 500 by 375 photo and its bounds;
  - preprocessing against the binary64 reference under every rule (within
    2 × 10⁻⁶) and against chapter 5's resize operators (identical);
  - the hand-worked convolutions, stride, padding, dilation and groups;
  - direct sum, im2col and gradients against central differences on 32
    settings, exact; chapter 5's `conv2d` on the setting it supports;
  - im2col against the direct sum on non-dyadic values, within one f32 step;
  - gradients by hand;
  - patch projection by hand, equal to the stride-P convolution, and every
    tile size equal to the direct sum; the plan's tile sizes at their bounds;
  - the lowering by hand, and the compiled program equal to the direct sum
    on all 32 settings and on horizontal dilation, on both of chapter 38's
    paths; the compiled patch projection equal to the tiled one;
  - position tables by hand and against chapter 5 through a transposed view;
  - every refusal at its bound, for each field alone.
- **Three examples** compared with their transcripts in debug and release.
- **Mutation testing** (`--root-only`, since chapters 4, 5 and 38 gate their
  own units) kills all 1,282 mutants of `pixels.coax`, run in two shards, and
  all 231 of `pixels-ir.coax`. The first runs left survivors, each fixed in
  the code or the checks:
  - comparisons at points where both sides agree, such as a clamp at 0 or the
    cubic kernel's knots at 1 and 2, rewritten without the comparison:
    `(u + |u|) / 2` for a clamp and truncated powers for the kernel;
  - a tile bound of exactly 2²⁰ that no admitted plan could tell from
    2²⁰ + 1, lowered to 2²⁰ − 1, which plans at 262,143 and 349,525 tokens
    tell apart;
  - a weight-zeroing pass hidden by zeroed scratch, replaced by a gather that
    writes every weight once, and checks that fill scratch with 7;
  - an oracle whose sign error cancelled between its two axes, now checked
    on one axis;
  - missing cases: tall images, a shorter side of 1, horizontal dilation,
    four-feature sum bounds;
  - from the earlier version: chapter 5 calls whose owner identities no
    input could observe, element limits no product of admitted extents could
    reach exactly, and limit checks a later check always caught.

## What this does not do yet

- **Chapter 5's operator table.** The convolution and patch projection run
  through chapter 38's IR, but they are not entries in chapter 5's operator
  table or its manifest, whose 27 entries and native checks belong to that
  chapter. A new entry there needs a new version of its contract.
- **GPUs.** Chapter 38 admits its convolution node on the CPU only, so the
  lowered programs do too.
- **Byte rounding.** PIL resizes 8-bit images in two passes and rounds to
  bytes after each; this chapter keeps f32 values. A model whose processor
  resizes bytes with PIL sees values up to about half a byte step apart.
- **Bicubic position tables.** Position tables resize by bilinear sampling
  only; many vision transformers use bicubic there.
- **No pooling here.** Chapter 5 already has max and average pooling with
  stride. Padded pooling is not built.

## Dependencies

- [Chapter 3](ch003-a-tensor-is-a-view-of-memory.md): views, used to read a
  position table feature-first in the checks.
- [Chapter 4](ch004-numbers-and-matrices.md): the compensated accumulator
  and the matrix product.
- [Chapter 5](ch005-a-standard-library-of-tensor-operators.md): the
  half-pixel resize arithmetic, and the `conv2d`, `nearest2d` and
  `bilinear2d` operators the checks compare against.
- [Chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md): the
  tensor IR, its compiler and its CPU paths.

## Sources

- [Chapter plan](PLAN.md), Part XIII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md).
- PyTorch's `upsample_bicubic2d` and antialiased resampling, PIL's
  `Resample.c`, and torchvision's `Resize` and `CenterCrop`, for the rules
  this chapter reproduces.
