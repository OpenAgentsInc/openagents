import{r as _}from"./index-76fb7be0.js";import"./_commonjsHelpers-de833af9.js";var d={exports:{}},s={};/**
 * @license React
 * react-jsx-runtime.production.min.js
 *
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */var f=_,y=Symbol.for("react.element"),x=Symbol.for("react.fragment"),v=Object.prototype.hasOwnProperty,E=f.__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED.ReactCurrentOwner,O={key:!0,ref:!0,__self:!0,__source:!0};function l(t,r,p){var e,o={},a=null,m=null;p!==void 0&&(a=""+p),r.key!==void 0&&(a=""+r.key),r.ref!==void 0&&(m=r.ref);for(e in r)v.call(r,e)&&!O.hasOwnProperty(e)&&(o[e]=r[e]);if(t&&t.defaultProps)for(e in r=t.defaultProps,r)o[e]===void 0&&(o[e]=r[e]);return{$$typeof:y,type:t,key:a,ref:m,props:o,_owner:E.current}}s.Fragment=x;s.jsx=l;s.jsxs=l;d.exports=s;var R=d.exports;const g=()=>R.jsx("p",{children:"I'm a node"}),S={title:"AgentGraph/Node",component:g,parameters:{layout:"centered"},tags:["autodocs"],argTypes:{}},n={args:{}};var c,i,u;n.parameters={...n.parameters,docs:{...(c=n.parameters)==null?void 0:c.docs,source:{originalSource:`{
  args: {
    // primary: true,
    // label: 'Button',
  }
}`,...(u=(i=n.parameters)==null?void 0:i.docs)==null?void 0:u.source}}};const b=["Primary"];export{n as Primary,b as __namedExportsOrder,S as default};
