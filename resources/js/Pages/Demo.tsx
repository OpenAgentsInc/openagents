import { FrameSVGNefrex } from "@arwes/react-frames";

export default function Demo() {
  return (
    <div
      style={{
        position: "relative",
        width: 300,
        height: 300,
      }}
    >
      <FrameSVGNefrex
        css={{
          "[data-name=bg]": {
            color: "hsl(180, 75%, 10%)",
          },
          "[data-name=line]": {
            color: "hsl(180, 75%, 50%)",
          },
        }}
      />
    </div>
  );
}
