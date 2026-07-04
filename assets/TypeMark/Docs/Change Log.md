## 1.13


[Detail](https://support.typora.io/What's-New-1.13/)

- Upgrade to MathJax v4.
- Upgrade to mermaid v11.13.0.
- When change markdown settings, user do not need to restart Typora. Now provide tooltip and button to reload Typora window to apply those changes.
- Better search performance.
- Improve UI, such as the source code mode button under macOS 26.
- Improve PDF export logic, now all features for PDF export is enabled under macOS 26.2.
- Better sidebar resize UI.
- Change “send anonymous info” settings from enabled by default to disabled by default for new installers.
- Fix insert content via macOS service not work when typora has data in clipboard.
- Fix a security issue.
- Fix UI stuck and menu gone when user modify export list in some cases.
- `<track>` element inside `<video>` now also respect relative path settings.
- Fix anchor location when click outline sidebar in exported HTML.
- Fix editor window not scroll sometimes when click online anchor.
- Fix indent rendering issue which affects lists and quotes.
- Fix a delete bug.
- Fix some HTML code not preserve when print to PDF.

## 1.12

[Detail](https://support.typora.io/What's-New-1.12/)

### New and Improvements

- Support for macOS 26 Tahoe
  - (macOS 26) Tahoe style icon with dark version and custom colors.
  - (macOS 26) Tahoe style menu.
  - (macOS 26) Fix ui compatibility for macOS 26.
  - (macOS 26) Remove “Window Style” option on macOS 26.
- Other bug fixes.

## 1.11

[Detail](https://support.typora.io/What's-New-1.11/)

- LaTeX Delimiters Support. Added support for `\(...\)` (inline math) and `\[...\]` (block math).
- Move Row/Paragraph Up/Down using `Alt + ↑ / ↓` to move.
- Mermaid v11.9 Update with new diagram types supported:
  - Radar Chart (`radar-beta`)
  - Treemap (`treemap-beta`)

### Improvements

- Exported HTML now includes Zenuml CSS only when needed (smaller file size).
- Improved math compatibility when pasting from ChatGPT and AI tools.
- Styles preserved when copying to WeChat Official Accounts.
- “Copy without theme styling” now respects plain text setting.
- Videos dropped into Typora now include the `controls` attribute.
- Exported images no longer use `referrerPolicy='no-referrer'` by default.
- Option added to disable emoji autocomplete when typing `:`.
- Improve UI translations for Slovenian, Chinese, Dutch, Polish, Norwegian Nynorsk, French, Portuguese and Swedish.
- Apple SF symbols can be used on built-in themes on macOS.

### Linux Updates

- Replaced old APT key using SHA1 with a new secure one. Follow https://support.typora.io/What's-New-1.11/#linux for more details
- Improved Wayland compatibility. Follow https://support.typora.io/What's-New-1.11/#wayland for more details

### Bug Fixes

- Fixed text clipping in PDF exports.
- Fixed Mermaid code not working on macOS 12.
- Fixed `---` not rendering properly.
- Prevented unwanted indentation when "Indent first line of paragraph" is enabled.
- Fixed issue where GIFs from clipboard couldn't be inserted.
- Fixed list paste issues in some cases.
- Fixed missing list bullets after 999+ items.
- Fixed PDF export glitches on macOS.
- Fixed issue where outline filter panel closed unexpectedly after scroll.
- Zoom setting with mouse wheel now preserved.
- Fixed rendering issues for certain code blocks with `url` mode.
- Fix sort naturally button in file sidebar.

## 1.10

[Detail](https://support.typora.io/What's-New-1.10/)

### Improvements

- Mermaid is now upgrade to 11.4, with new diagrams including:   
  - Packet Diagram
  - Kanban Diagram
  - Architecture Diagrams
- Update translations for Danish, Hindi and Vietnamese.
- Better PDF export on Windows / Linux: Able to export to PDF with dark themes and keep the background.
- Toggle alert from View menu now follows the same logic as blockquote, which means you can select a range of paragraph and changes them into Github Alert from Paragraph → Alert from menubar.
- Add `gas` syntax highlight, in previous version, `gas` will be treated as alias of assembly incorrectly.
- Add `url` syntax highlight.
- The image uploader option, `piclist`, is applied on all languages.
- Fix syntax switch for images failed in some cases.
- Allow users to drag-and-drop / insert images with jiff extension.
- Do not escape image url with `http` protocol.

### Fix

- Fix window fullscreen / maximize state not restored on Typora restart.
- Allow scroll the side mega menu panel when zoomed under Windows.
- Fix file contains some emojis are not detected as UTF-8 encode
- Fix select lists items and toggle list type be applied on wrong blocks.
- Fix typing on last line of code block may make the app hangs.
- Fix adding anchor link to headings with "+" character not work correctly after export.

## 1.9

[Detail](https://support.typora.io/What's-New-1.9/)

### New

- Support gitlab style math code block.
- Upgrade mermaid 10.9.1, Block Diagrams are now supported.

### Improvements

- Follow CommonMark rule to handle whitespace inside code.
- Improve scroll performance, improve performance on macOS 14.
- Shorten file path in open recent menu.
- "Inspect Element" on macOS moved from context menu to Sfarai Develop menu.
- New file on file tree no longer need confirm after input its filename.
- Add structured text code syntax highlight.
- Chapter levels can be configured for ePub export.
- Update languages in Polish, Traditional Chinese, Slovenian, Czech and Norwegian Nynorsk.

### Fix

- Fix Javascript XSS on mermaid and math.
- Fix anchor id when export.
- Fix bookmark for exported PDF cannot be shown in Adobe Reader.
- Fix some remote images cannot be loaded on macOS.
- Fix "&" not displayed for file path contains "&" In "recent files" menu on Windows / Linux.
- Fix "$" in code language name leads to a wrong string.
- Other bug fix.

## 1.8

[Detail](https://support.typora.io/What's-New-1.8/)

#### New

- Support github style alert.
- Upgrade mermaidjs, added xychart diagram.
- Add options to zoom Typora when using mouse wheel and press command/ctrl key.

#### Improvements and Fix

- Add option to add "New Markdown" under context menu → "New" sub menu of Windows Explorer.
- Performance improvement.
- Support dark mode detect on Linux.

### Fix

- Fix save as image for diagrams.
- Hide "copy as HTML" for unsupported blocks.
- Add "copy as HTML" support for mermaid diagrams.
- Fix "PicList not found" error when using PicList to upload images.
- Fix window position in multiple monitor.

## 1.7

[Detail](https://support.typora.io/What's-New-1.7/)

#### New

- Support Quadrant Chart, Sankey diagrams and zenuml for mermaid diagarms.
- Add menu item to open outline popover (macOS) and Word Count popover.
- Larger scrollbar when hover on Windows/ Linux.
- Add Modelica language syntax support.
- Add delete diagram option in context menu.
- Add options to not save recent folders / files.
- Add buttons to pin and remove recent folders from sidebar panel.
- Add variable `${outputFolder}` and `${outputFolderName}`

#### Improvements and Fix

- Better paste list from Apple Notes.
- Improve open file with anchor locations.
- Fix sort order not saved after restart Typora.
- Header and footers now respect page margins when export to PDF.
- Fix exporting ordered list when using theme Whitey.
- `typora-root-url` now supports absolute image path on Windows.
- Fix filename suggestion is improper when first heading is under editing.
- Fix indent in code blocks not kept when paste into other web apps on macOS.
- Non-exist `#anchor` links is now preserved.
- Disable `alert` in embeded iframes.
- Fix shortcut key for upload image cannot be set.
- Fix drag files from file sidebar on some Linux distributions.
- Fix app not responsive when do line-wise cut on empty line.
- Fix some issues about cut line.
- Fix indent issue when indent paragraph is enabled on macOS ≥ 13
- Fix improper behavior when press shift + arrow key then press arrow key on macOS.
- Fix typo of the menu item for Greek encoding option on Windows / Linux.
- Fix code language autocomplete not work for `~~~`.
- Fix security issue STAR-2023-0062
- Add menu item of "jump to line start", fix Ctrl + A/E on macOS.
- Add batch as alias of `BAT` in code language selector.
- Fix some meta data on YAML not get read or applied on export.
- Fix a bug that create new file on file list panel will use content of currently opened file, instead of an empty file.
- Other bug fix.


## 1.6

[Detail](https://support.typora.io/What's-New-1.6/)

#### New

- Add "Files" section and retune Preferences Panel.
- Add options to set default file extension.
- Add options to control behavior when drop file or folder into Typora.
- Add options to disable auto link.
- Add options to set default code language when inserting code blocks.
- Add options to auto apply last used code language when inserting code blocks.
- Support Timeline for MermaidJS diagrams.
- Add Thai Language UI.
- Add PicList as an Image Uploader.
- Add PEG.js syntax highlight support.

#### Fix

- Fix CVE-2023-2317 and CVE-2023-2316.
- Avoid using google font mirrors in exported HTML, now official google font CDN is used.
- using keyword["keyword"] sometimes causes node not rendered in Mermaid diagram.
- color of typic package not readable in dark themes.
- setting inline mermaid configs may also affect other mermaid blocks.
- Tasks status cannot be changed from menu bar when task list is under nested lists.
- math block is always auto numbered in exported docx and related setting is ignored.
- [Spec change] Typora now allow users to escape : mark to avoid input unwanted emoji codes.

#### Fix from 1.5.5 to 1.5.14

- Fix glitch of search icon
- Fix Fuzzy search on quick open panel (macOS)
- Fix anchor link in exported HTML.
- Fix image layout on macOS 13.3
- Fix selection range after insert image.
- Fix cursor when typing inside styled links.
- Fix find again after switch docs.
- Fix use command line typora aaa.md when aaa.md does not exists.
- Fix garbled path under open recent menu.
- Fix PDF export contains page break div.
- Add option to switch Shift+Tab behavior between outdent and auto indent inside code blocks.
- Add menu and context menu to copy all code contents.
- Add menu and context menu to auto indent code contents.
- Fix sort in file tree not saved.
- Fix compatibility issue with Pandoc 3.0
- Fix the behavior of opening folder in Typora.
- Fix create new file in file tree.
- Fix footer and header for exported PDF.
- Other bug fixes.

## 1.5

[Detail](https://support.typora.io/What's-New-1.5/)

- Add yara, svelte code syntax highlight, improve sql language syntax.
- Shift + Tab will outdent selected contents in code block and source code mode.
- Improve syntax highlight in source code mode.
- Improve global search results.
- Upgrade mermaid version which brings bug fix and Mindmaps.
- Highlight is supported and converted when export to LaTeX.
- Improve export functions.
- Allow open local file link contains “#” in file path.
- Improve paste results from Google Docs.
- Add new UI language: Norwegian, and update some other language translations.
- Fix issues about HTML entities.
- Fix focus loss when create new tabs of Typora window.
- Other bug fixes and improvements.

## 1.4

[Detail](https://support.typora.io/What's-New-1.4/)

#### New

- Upgrade mermaid version. Support inline mermaid configs.
- Upgrade mathjax version.
- Add copy as image option for math and diagrams.
- Add option to mix files and folders when sorting in file tree.
- Add sort by creation time option for file tree / file list.
- File tree view and file list view now is configured separately.
- Increase max line count for global search results.
- Fuzzy search in `Open Quickly` will now also match folder path.
- Add new math auto numbering rule — AMS Numbering Rules (where only certain environments produce numbered equations, as they would be in LaTeX).
- Add option to always add “./” when generating relative path for images.
- Add `rename image` in context menu. (Which is the same menu item with `move images`).
- Improve Pascal lang support, add more keyword highlight for Pascal.
- Add syntax support for Smarty.
- Improve performance when using find function and prevent hangs.
- Make search result more readable compared to `==highlight==` texts.
- Increase max col count in table edit UI.
- Recognize .qmd as markdown file.
- Now user can select multiple cols and apply text align from table tool tip all at once.

#### Fix

- Fix bugs about switching files.
- Fix triple click on code block.
- Fix code block height not auto changed when enter or exit fullscreen mode on macOS.
- Make font color of completed task more readable in dark themes
- Fix some rendering issue for mermaidjs. 
- Remove unnecessary horizontal scroll bar on side bar. 
- Fix undo deleted file not displayed in file tree view.
- Fix error when switch files.
- Fix hint behavior when hover on folders of sidebar menu.
- Use smaller margin between math block and normal paragraphs.
- Fix compatibility issues when search with Tai.
- PDF export now supports headers or footers contains non-ASCII code when export.
- Improve image export quantity.
- Fix PDF export not working on Windows when user account contains special characters.
- Fix epub export show warnings in epub reader when contains video tag
- Fix page size setting of PDF not working on macOS when export.
- Fix the setting of margin right and margin left of PDF not correctly applied when export.
- Fix last export path not saved on macOS.
- Fix docx import fail on newer pandoc versions.
- Fix replace may ignore search results matches with regular expressions contains non-capture groups.
- Fix move images not working for windows.
- Improve performance when md file contains `video` tag.
- Fix a caret jump issue when typing after inline math.
- Fix minor issues with spellcheck.
- Fix load image with src contains escaped @ mark.
- Fix save-as sometimes not popped up on diagrams.

## 1.3

[Detail](https://support.typora.io/What's-New-1.3/)

#### New

- Support copy / move / download all images.
- Add advanced copy / paste action in context menu.
- Support regular expression in file search, find and replace.
- Show match counts in find and replace panel.
- Support auto move image folder when move or rename current opened markdown document.
- Support ${currentFolder} as export command variable.
- Add Hindi language support.

#### Fix

- Update translations for Chinese, Polish, Slovenian, Spanish, Arabic, Indonesian, Portuguese, etc.
- Update image and math for replace operation, fix search with html entities.
- Fix learned word not kept in some cases (Window / Linux).
- Support abbreviation like (e.g.) is always marked as misspelled in Window / Linux.
- Reduce laggy when editing file with large set of math expressions.
- Do not auto capitalize words after and inside inline math.
- Fix some bugs about text selection.
- Fix poster support for video element.
- Fix live rendering for footnote.
- Fix parsing rules about hr in lists.
- Fix copy table header may include control elements.
- Fix display issue for code block line number when window resize and export.
- Fix issues about open links to other file with anchor position.
- Fix select line at first char of line.
- Keep unicode quote when import.
- Fix context menu for code fences.
- Fix PDF export pay produce empty page as the last page.
- Other bug fix and performance improvement

## 1.2

[Detail](https://support.typora.io/What's-New-1.2/)

- Upgrade Mermaid library version.
- Support move and delete images.
- Undo a copy image operation will now also remove the copied image.
- Add new image uploader Picsee (macOS).
- Improve performance when document contains large set of images (macOS).
- Better encode detection (Windows / Linux).
- Add docker, py and code language alias, add stata, postgresql, hive syntax highlight.
- Other bug fix.

## 1.1

[Detail](https://support.typora.io/What's-New-1.1/)

#### Improvements

- Add Malay language support.
- Add Slovenian language support.
- Fix typo and update translations in other languages.
- Allow create file when open non-exist file via command line.
- Allow create file when open target link that does not have existing file. 
- Allow open other files' anchor position directly via file link.
- More compact context menu under Windows / Linux.
- Support copy / cut while line when nothing is selected.
- Add option to disable or enable physics package.
- Allow pasting markdown with Command-Option-Shift-V on macOS.
- Support offline activation and provide web UI to manage used license code.
- Sign Windows executables and installers.
- Support default / theme font size for exported image.
- Ensure downloaded image has image file extension.
- Support loading local image files with user append hashes.
- Improve delete line and delete block logic.
- Improve delete current block and delete current line.
- Assign default shortcut keys for select range and delete range actions.

#### Fix

- Fix security issue which may cause XSS.
- Fix custom font size for exported image. 
- Fix multiple BOM be added when saving files with BOM header.
- Fix copy as MathML not working for math block.
- Fix math auto numbering get wrongly numbered labels.
- Fix file tree UI under smaller customer font size.
- Fix always on top cannot be turned off under macOS.

## 1.0

🎉🎉🎉🎉🎉🎉🎉🎉🎉

[Detail](https://support.typora.io/What's-New-1.0/)

Typora is now finally out of beta, Thanks for all your support!

## 0.11.0 (beta)

[Detail](https://support.typora.io/What's-New-0.11/)

## 0.10.0 (beta)

[Detail](https://support.typora.io/What's-New-0.10/)