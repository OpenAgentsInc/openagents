use mermaid_viewer::render_mermaid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A tiny themed SVG sample resembling a simple flow.
    // In real use, pass an SVG produced from Mermaid.
    const SVG: &str = r#"
<svg xmlns='http://www.w3.org/2000/svg' width='1000' height='700' viewBox='0 0 1000 700'>
  <defs>
    <marker id='arrow' viewBox='0 0 10 10' refX='10' refY='5' markerWidth='7' markerHeight='7' orient='auto-start-reverse'>
      <path d='M 0 0 L 10 5 L 0 10 z' fill='#7a7f85'/>
    </marker>
  </defs>
  <rect x='0' y='0' width='1000' height='700' fill='#08090a'/>

  <g stroke='#6c7075' stroke-width='1.25' fill='none'>
    <rect x='140' y='120' width='180' height='48'/>
    <text x='230' y='150' text-anchor='middle' fill='#f7f8f8' font-size='14'>Start</text>

    <rect x='420' y='120' width='220' height='48'/>
    <text x='530' y='150' text-anchor='middle' fill='#f7f8f8' font-size='14'>Process A</text>

    <rect x='420' y='260' width='220' height='48'/>
    <text x='530' y='290' text-anchor='middle' fill='#f7f8f8' font-size='14'>Process B</text>

    <rect x='760' y='260' width='120' height='48'/>
    <text x='820' y='290' text-anchor='middle' fill='#f7f8f8' font-size='14'>End</text>

    <!-- Edges -->
    <line x1='320' y1='144' x2='420' y2='144' stroke='#7a7f85' marker-end='url(#arrow)'/>
    <line x1='530' y1='168' x2='530' y2='260' stroke='#7a7f85' marker-end='url(#arrow)'/>
    <line x1='640' y1='284' x2='760' y2='284' stroke='#7a7f85' marker-end='url(#arrow)'/>
  </g>
</svg>
"#;

    let viewer = render_mermaid(SVG)?;
    viewer.run()?;
    Ok(())
}

