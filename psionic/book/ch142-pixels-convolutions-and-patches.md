# 142. Pixels, convolutions and patches

> **Status: partly implemented.** Image admission, preprocessing profiles,
> nearest and bilinear resizing, 2-D convolution with stride, padding,
> dilation and groups (forward two ways, and its gradients), patch projection
> and position-table resizing run on the CPU. Bicubic resizing, other
> corner-alignment rules, convolution and patch operators in chapter 5's
> operator registry, and their lowering into chapter 38's tensor IR are not
> built yet.

Every vision model in this book starts by turning an image file into a
tensor with exact conventions. A pixel range of `[0, 1]` instead of
`[0, 255]`, another resize filter or a swapped channel order changes every
value downstream, and the model still produces confident output. This chapter
writes those conventions down as data, builds the operations that Part XIII's
models share, and checks each one on fixtures small enough to verify by hand.

It builds five things, in [`pixels.coax`](../lib/pixels.coax):
- a **preprocessing profile**: channel order, value scale, per-channel mean
  and spread, output size and resize filter, and the procedure that turns
  8-bit RGB bytes into a normalized channel-major tensor;
- **2-D convolution** with stride, padding, dilation and groups, computed by
  a direct sum and again by image-to-columns plus
  [chapter 4](ch004-numbers-and-matrices.md)'s matrix product;
- the convolution's **gradients** for the input, the weights and the bias;
- **patch projection**, the first layer of a vision transformer;
- **position-table resizing**, for running such a model at a new image size.

```shell
coax/bin/coax run psionic/examples/pixels.coax
coax/bin/coax run psionic/examples/pixels-refuse.coax
psionic/check.sh --only pixels
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
PxFilter :: enum u8 {nearest;bilinear;}
// Pixels arrive row by row, each pixel's channels together (height, width,
// channel), one byte per channel, in the stated channel order.
PxImage :: struct {width:int;height:int;channels:int=3;depth:int=8;order:PxOrder;}
// What a model expects: channel order, the factor that maps a byte to its
// range, per-channel mean and standard deviation, and the resize rule.
PxProfile :: struct {order:PxOrder;scale:f64;mean:[3]f64;std:[3]f64;height:int;width:int;filter:PxFilter;}
PxResult :: struct {status:PxStatus;written:int;}
// Input (batch, channels, height, width); weights (filters, channels/groups,
// kernel_h, kernel_w); output (batch, filters, out_h, out_w).
PxConv :: struct {batch:int=1;channels:int;height:int;width:int;filters:int;kernel_h:int;kernel_w:int;
    stride_h:int=1;stride_w:int=1;pad_h:int;pad_w:int;dilation_h:int=1;dilation_w:int=1;groups:int=1;}
PxConvPlan :: struct {status:PxStatus;out_h:int;out_w:int;x:int;w:int;y:int;patch:int;columns:int;work:int;scratch:int;}
// Input (batch, channels, height, width); weights (dim, channels, patch,
// patch), the layout of the equivalent convolution; output (batch, tokens, dim).
PxPatch :: struct {batch:int=1;channels:int;height:int;width:int;patch:int;dim:int;}
PxPatchPlan :: struct {status:PxStatus;grid_h:int;grid_w:int;tokens:int;features:int;x:int;w:int;out:int;scratch:int;}
```

## A preprocessing profile

A model's preprocessing is part of the model. Its training code fixed:
- the **channel order**: RGB, or BGR for models trained on images decoded
  with OpenCV;
- the **scale** that maps a byte to the model's range, 1/255 for `[0, 1]`;
- the **mean** subtracted from each channel and the **spread** each is
  divided by, after scaling;
- the **output size** and the **resize filter**.

The profile record holds exactly those fields. Admission refuses an image
with a zero side, a side above 512, any channel count other than three or a
depth other than 8 bits, each with its own status. It refuses a profile whose
scale is outside (0, 1], whose mean is outside [−256, 256] or whose spread is
outside [2⁻¹⁰, 256]. NaN fails every one of those comparisons, so it is
refused too.

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
scales them to f32, resizes each plane and normalizes. Each stage rounds to
f32 once, as chapter 5's operators would. The checks run chapter 5's
`nearest2d` and `bilinear2d` on the same scaled planes and require every
output to be identical.

<!-- source: psionic/lib/pixels.coax preprocess -->
```jai
px_preprocess_work :: (image:PxImage,p:PxProfile)->int {return 3*image.width*image.height;}
// Bytes to model values: pick the model's channel order and lay the image out
// channel by channel, times the scale; resize; subtract the mean and divide by
// the spread. Each stage rounds to f32 once, as chapter 5's operators do.
px_preprocess :: (image:PxImage,pixels:[]u8,p:PxProfile,work:[]f32,out:[]f32,overlap:bool)->PxResult {
    s:=px_image_status(image);if s!=PxStatus.ok {return PxResult.{status=s};}
    s=px_profile_status(p);if s!=PxStatus.ok {return PxResult.{status=s};}
    h:=image.height;w:=image.width;area:=h*w;size:=p.height*p.width;
    if pixels.count!=3*area {return PxResult.{status=PxStatus.shape};}
    if work.count<3*area || out.count<3*size {return PxResult.{status=PxStatus.capacity};}
    if overlap {return PxResult.{status=PxStatus.alias};}
    for c:0..<3 {
        // Model channel c is source channel c, or 2 - c when the orders differ.
        source:=c;if image.order!=p.order {source=2-c;}
        for i:0..<area {work[c*area+i]=f32(f64(pixels[i*3+source])*p.scale);}
    }
    for c:0..<3 {for y:0..<p.height {for x:0..<p.width {
        v:=f32(px_sample(work,c*area,w,1,h,w,p.height,p.width,p.filter,y,x));
        out[(c*p.height+y)*p.width+x]=f32((f64(v)-p.mean[c])/p.std[c]);
    }}}
    return PxResult.{written=3*size};
}
```

## An independent reference

The reference below works straight from the bytes in binary64. It writes the
bilinear rule the way image libraries document it, with floating-point
source coordinates, a floor and a clamp, instead of chapter 5's integer
form. The clamp at the top edge is applied to the second neighbour only:
when the source point passes the last pixel, both neighbours are the last
pixel and the fraction does not matter.

<!-- source: psionic/lib/pixels.coax reference -->
```jai
// One output value straight from the bytes, in binary64, written from the
// usual description of bilinear sampling with half-pixel centres: the source
// point is (out + 1/2) * in / size - 1/2, at least 0; the pixel at its floor
// and the next one, which stops at the last pixel, are blended by the
// fraction. Nearest reads source floor(out * in / size).
px_reference :: (image:PxImage,pixels:[]u8,p:PxProfile,c:int,y:int,x:int)->f64 {
    source:=c;if image.order!=p.order {source=2-c;}
    h:=image.height;w:=image.width;v:=0.0;
    if p.filter==PxFilter.nearest {v=f64(pixels[((y*h/p.height)*w+x*w/p.width)*3+source]);}
    else {
        sy:=(f64(y)+0.5)*f64(h)/f64(p.height)-0.5;sx:=(f64(x)+0.5)*f64(w)/f64(p.width)-0.5;
        sy=(sy+abs(sy))/2;sx=(sx+abs(sx))/2;
        y0:=int(floor(sy));x0:=int(floor(sx));y1:=y0+1;x1:=x0+1;
        if y1==h {y1=h-1;}
        if x1==w {x1=w-1;}
        fy:=sy-f64(y0);fx:=sx-f64(x0);
        top:=(1-fx)*f64(pixels[(y0*w+x0)*3+source])+fx*f64(pixels[(y0*w+x1)*3+source]);
        bottom:=(1-fx)*f64(pixels[(y1*w+x0)*3+source])+fx*f64(pixels[(y1*w+x1)*3+source]);
        v=(1-fy)*top+fy*bottom;
    }
    return (v*p.scale-p.mean[c])/p.std[c];
}
```

On a 7 by 5 image resized to 9 by 4 under ImageNet's profile, the largest
difference between `px_preprocess` and the reference is 1.97 × 10⁻⁷. The f32
roundings of the scaled byte, the resized value and the normalized value
account for it: one f32 step at 2 is 2.4 × 10⁻⁷. The checks require at most
2 × 10⁻⁶ on that image and three more sizes, with both filters and both
channel orders.

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

<!-- source: psionic/lib/pixels.coax patch -->
```jai
// Chapter 4's product bounds the projection at 2^20 multiply-adds, and with
// it every count below.
px_patch_plan :: (q:PxPatch)->PxPatchPlan {
    extents:=[6]int.[q.batch,q.channels,q.height,q.width,q.patch,q.dim];
    for e:extents {if e<1 {return PxPatchPlan.{status=PxStatus.shape};}}
    for e:extents {if e>PX_EXTENT_MAX {return PxPatchPlan.{status=PxStatus.limit};}}
    if q.height%q.patch!=0 || q.width%q.patch!=0 {return PxPatchPlan.{status=PxStatus.shape};}
    p:=PxPatchPlan.{grid_h=q.height/q.patch,grid_w=q.width/q.patch};
    p.tokens=p.grid_h*p.grid_w;p.features=q.channels*q.patch*q.patch;
    request:=PsionicRequest.{op=PsionicOp.matmul,rows=q.batch*p.tokens,width=p.features,columns=q.dim};
    if psionic_ops_plan(request).status!=PsionicOpStatus.ok {return PxPatchPlan.{status=PxStatus.limit};}
    p.x=q.batch*q.channels*q.height*q.width;p.w=q.dim*p.features;p.out=q.batch*p.tokens*q.dim;
    p.scratch=p.x+p.w+2*p.out;
    return p;
}
// Cut each image into patch by patch squares, left to right and top to
// bottom; each square becomes one row of features in (channel, row, column)
// order, the order of the convolution's weights.
px_patchify :: (q:PxPatch,x:[]f32,tokens:[]f32)->int {
    p:=px_patch_plan(q);if p.status!=PxStatus.ok || x.count!=p.x || tokens.count<p.x {return 0;}
    for r:0..<q.batch*p.tokens {
        n:=r/p.tokens;gy:=(r%p.tokens)/p.grid_w;gx:=r%p.grid_w;
        for k:0..<p.features {
            ch:=k/(q.patch*q.patch);dy:=(k/q.patch)%q.patch;dx:=k%q.patch;
            tokens[r*p.features+k]=x[((n*q.channels+ch)*q.height+gy*q.patch+dy)*q.width+gx*q.patch+dx];
        }
    }
    return p.x;
}
// Tokens times the transposed weights, plus the bias: the first layer of a
// vision transformer. Scratch holds the tokens, the transposed weights,
// chapter 4's staging and the product, in that order.
px_patch_embed :: (q:PxPatch,x:[]f32,w:[]f32,bias:[]f32,work:[]f32,out:[]f32,overlap:bool)->PxResult {
    p:=px_patch_plan(q);if p.status!=PxStatus.ok {return PxResult.{status=p.status};}
    if x.count!=p.x || w.count!=p.w || bias.count!=q.dim {return PxResult.{status=PxStatus.shape};}
    if work.count<p.scratch || out.count<p.out {return PxResult.{status=PxStatus.capacity};}
    if overlap {return PxResult.{status=PxStatus.alias};}
    // Chapter 4 checks the pixels and weights it multiplies.
    if !px_finite(bias) {return PxResult.{status=PxStatus.number};}
    #assert px_patchify(q,x,work[0..<p.x])==p.x;
    for k:0..<p.features {for d:0..<q.dim {work[p.x+k*q.dim+d]=w[d*p.features+k];}}
    idx:[1]int;mask:[1]u8;stage:=p.x+p.w;product:=stage+p.out;
    r:=psionic_ops_checked(PsionicRequest.{op=PsionicOp.matmul,rows=q.batch*p.tokens,width=p.features,columns=q.dim},work[0..<p.x],
        work[p.x..<stage],idx[0..<0],mask[0..<0],work[product..<product+p.out],work[stage..<product],false);
    if r.status!=PsionicOpStatus.ok {return PxResult.{status=PxStatus.number};}
    for i:0..<p.out {
        v:=f64(work[product+i])+f64(bias[i%q.dim]);
        if !psionic_ops_fits_f32(v) {return PxResult.{status=PxStatus.number};}
        work[product+i]=f32(v);
    }
    for i:0..<p.out {out[i]=work[product+i];}
    return PxResult.{written=p.out};
}
```

Cutting into squares and multiplying is a convolution whose kernel and
stride are both P. The weights, stored as (dim, channels, P, P), are the
convolution's weights. The example runs the same image as a stride-2
convolution. It prints the same four numbers in the convolution's order,
filter before position. The checks require equality on every element for
patch sizes 1 to 3 and batches of 1 and 2.

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
    q=PxPatch.{channels=4,height=512,width=512,patch=1,dim=2};print("a projection of 2^21 multiply-adds: %\n",px_patch_plan(q).status);
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
a projection of 2^21 multiply-adds: limit
a position table resized to 0 rows: parameter
3 table values for a 2x2 grid: shape
```

## Supported profile

`psionic-pixels-v1`, bounded CPU:
- 8-bit, three-channel images up to 512 by 512, in RGB or BGR order;
  profiles with scale in (0, 1], means within [−256, 256] and spreads within
  [2⁻¹⁰, 256], output sides up to 512;
- nearest and half-pixel bilinear resizing, exactly as chapter 5's operators;
- convolution extents, strides and dilations up to 4,096, padding below the
  kernel's span, groups that divide both channel counts;
- at most 2²⁰ values per tensor and 2²⁴ multiply-adds forward and for the
  input gradient; im2col's scratch at most 2²⁰ values, and each group's
  product at most 2²⁰ multiply-adds (chapter 4's bound);
- patch projections of at most 2²⁰ multiply-adds (chapter 4's bound);
- f32 storage, binary64 sums with chapter 4's compensated accumulator, one
  rounding to f32 per stored value.

## How it is checked

- **Coax checks:** 21 checks in `pixels.coax`, 91 counting the units it
  loads, in debug and release:
  - the hand-worked resize, both filters, both channel orders;
  - preprocessing against the binary64 reference (within 2 × 10⁻⁶) and
    against chapter 5's resize operators (identical);
  - the hand-worked convolutions, stride, padding, dilation and groups;
  - direct sum, im2col and gradients against central differences on 32
    settings, exact; chapter 5's `conv2d` on the setting it supports;
  - im2col against the direct sum on non-dyadic values, within one f32 step;
  - gradients by hand;
  - patch projection by hand and equal to the stride-P convolution;
  - position tables by hand and against chapter 5 through a transposed view;
  - every refusal at its bound, for each field alone.
- **Two examples** compared with their transcripts in debug and release.
- **Mutation testing** of `pixels.coax` (`--root-only`, since chapters 4 and
  5 gate their own units) kills all 901 mutants. The first runs left
  survivors, each fixed in the code or the checks:
  - chapter 5 calls in the library had owner identities and view flags that
    no input can observe, so the library reuses chapter 5's `to_point`
    arithmetic and the checks call chapter 5's operators;
  - element limits passed at every call site could not be reached exactly
    by any product of admitted extents, so one procedure holds each bound and
    is checked at 2²⁰ and 2²⁰ + 1 directly;
  - limit checks that a later check always caught, rewritten so each limit
    is reached on its own;
  - a fit check that only re-read values, replaced by an in-place pass;
  - missing checks at exact bounds (a width of 4,096, a profile side of 512).

## What this does not do yet

- **Bicubic resizing and other alignment rules.** Many vision transformers
  resize images and position tables with bicubic filters, and some with
  `align_corners=True` or antialiasing. Only nearest and half-pixel bilinear
  run. A profile also takes its output size as given; aspect-preserving resize
  and crop rules are not written down yet.
- **The operator registry and the tensor IR.** The convolution and patch
  procedures are not entries in chapter 5's registry, and nothing here lowers
  into [chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md)'s
  tensor IR or a GPU.
- **Sizes.** Chapter 4's product limits a patch projection to 2²⁰
  multiply-adds. A 224-pixel ViT-B/16 projection needs about 1.2 × 10⁸.
  [Chapter 143](ch143-elephant-and-mammoth-see.md) and
  [chapter 144](ch144-see-with-c-radio.md) need a larger product.
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

## Sources

- [Chapter plan](PLAN.md), Part XIII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md).
