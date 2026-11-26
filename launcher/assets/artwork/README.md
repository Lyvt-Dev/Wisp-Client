# Wisp 0.6.0 artwork

Created with the built-in image_gen tool using the owner's new UI reference. The artwork contains scenery only; account names, avatars, versions, instances and game status are rendered from application state.

Project assets: launcher/assets/artwork/wisp-hero.png and launcher/assets/artwork/wisp-news.png.

Delivered assets: outputs/artwork/wisp-reference-hero.png and outputs/artwork/wisp-reference-news.png.

ribbon.svg and sidebar-mountains.svg are code-native decorative assets. The folded-A brand mark remains the existing Wisp artwork.

## Hero prompt

Use case: precise-object-edit. Asset type: production scenic hero background for Wisp native launcher. Input image is the user's authoritative UI reference. Reconstruct ONLY the landscape artwork inside the large top-center hero panel, as a clean full-bleed extra-wide 2.53:1 image. Preserve as closely as possible that scene: dark voxel spruce forest on the left, snow-tipped angular block mountains at center, tranquil lake reflecting sunset, square golden sun right of center, lavender block clouds, luminous pink cherry trees lining the right shore. Violet dusk shadows, warm amber-pink sunset, polished voxel shader rendering. Remove ALL interface, text, typography, logos, buttons, pagination dots, borders and other panels. The left half should retain its dark forest detail and space for app text. Image is artwork only, no user identity, no watermark. This is an asset extraction/reconstruction, not another launcher mockup.

## News prompt

Use case: precise-object-edit. Asset type: production update-card illustration for Wisp native launcher. Input image is the user's authoritative UI reference. Reconstruct ONLY the small landscape picture in the What's new panel at lower-left center: glowing purple rectangular voxel portal in a lush forest clearing, dark blocky trees and branches framing it, square blue moon in an indigo night sky, soft amber lanterns, flowers and lake reflections. Match the reference picture's composition and saturated purple/blue lighting as closely as possible. Output a clean full-bleed 1.8:1 landscape illustration. Remove ALL interface, text, labels, logo, buttons, frames, borders, account data and neighboring panels. Artwork only, no watermark.
# Aurora preview

`aurora-preview.png` is an unedited native Vulkan screenshot of the original
Wisp Aurora 1.0.0 shader, captured on 5 November 2026 in a fresh isolated world
(seed 1234, forest pond near -1100, -560). It shows actual shader output, including
water reflections; it is not generated concept art. The test used a scratch copy
of locally prepared Minecraft resources. No raw resources or player data are
included. Other scenic catalog/header artwork remains illustration.
