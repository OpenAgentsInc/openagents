// Load icon fonts for React Native Vector Icons in Electron
// Using a string path to avoid TypeScript errors with font imports

// @ts-ignore - We're hardcoding the path to the font file to avoid TypeScript errors
const IoniconsFont = '/node_modules/react-native-vector-icons/Fonts/Ionicons.ttf';

// Create a style element to load the icon font
const iconFontStyles = `
@font-face {
  font-family: "Ionicons";
  src: url(${IoniconsFont}) format("truetype");
  font-weight: normal;
  font-style: normal;
}
`;

// Inject the styles into the document
export const loadIconFonts = () => {
  // Don't add the styles if they're already present
  if (document.getElementById('ionicons-font-styles')) return;

  const style = document.createElement('style');
  style.id = 'ionicons-font-styles';
  style.type = 'text/css';
  style.appendChild(document.createTextNode(iconFontStyles));
  document.head.appendChild(style);
};

export default loadIconFonts;