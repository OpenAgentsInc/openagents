import React, { useState, useEffect } from "react";
import { Animator } from "@arwes/react-animator";
import { Dots } from "@arwes/react-bgs";
import { Head } from "@inertiajs/react";

export default function Demo() {
  const [active, setActive] = useState(true);

  useEffect(() => {
    const iid = setInterval(() => setActive((active) => !active), 3000);
    return () => clearInterval(iid);
  }, []);

  return (
    <>
      <Head title="Welcome" />
      <Animator active={active} duration={{ enter: 2, exit: 2 }}>
        <div
          style={{
            position: "relative",
            width: "100vw",
            height: "100vh",
            backgroundColor: "black",
          }}
        >
          {/* Canvas element will ocupy the positioned parent element. */}
          <Dots color="rgba(255,255,255,0.25)" />
          <iframe
            src="https://wanix.openagents.com"
            title="Example iframe"
            width="100%"
            height="100%"
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              border: "none",
            }}
          />
        </div>
      </Animator>
    </>
  );
}
