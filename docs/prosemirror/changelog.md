# ProseMirror Changelog Summary

## Major Recent Changes

### Version 1.40.0 (2025-06-02)
#### New Features
- `"handleTextInput"` prop now takes an additional argument for generating default text change transactions
- Widget decorations support a `relaxedSide` option to allow DOM cursor placement on either side

### Version 1.39.0 (2025-04-04)
#### New Features
- Added `dragCopies` prop to configure whether drag events inside the editor copy or move content

### Version 1.38.0 (2025-02-12)
#### New Features
- Clipboard serialization logic now available as `serializeToClipboard` method on view object

## Significant Improvements

### Typescript Support
- Most ProseMirror packages now include TypeScript type declarations
- Added optional type parameters to `Schema` for node and mark names

### Markdown and Parsing Enhancements
- Markdown serializer now supports more flexible escaping and configuration
- Improved DOM parsing with better mark and whitespace handling
- Added support for custom token encoding in change sets

### View and Interaction Improvements
- Better handling of compositions, especially on mobile devices
- Improved cursor and selection management across different browsers
- Enhanced support for decorations and node views

## Notable Bug Fixes

- Fixed issues with cursor placement on various platforms
- Resolved problems with mark application during parsing
- Improved compatibility with different input methods and keyboard layouts
- Fixed edge cases in document transformation and selection

## Migration Considerations

- Check type declarations when upgrading
- Review changes to props like `handleTextInput` and `dragCopies`
- Test clipboard and drag-and-drop interactions thoroughly
- Verify markdown serialization behavior if customized