# NieR Re[in]carnation - le launcher pour Linux

[English version](README.md)

## En deux mots

Vous jouez à NieR Re[in]carnation sur PC grâce au portage d'AllenWalkerED, et vous êtes sous Linux ?
Ce launcher remplace celui de Windows. Il s'ouvre comme n'importe quel programme Linux, démarre le
serveur local, prépare le jeu, puis ne confie à Wine ou Proton que ce qui en a vraiment besoin :
le runtime du jeu.

Version 1.0.0, par **AnnoyedDev**.

## Ce que j'y ai mis de personnel

Trois choses le distinguent du launcher d'origine :

1. Il parle français. Le choix se fait dans Paramètres > Langue > Launcher, et l'anglais reste là.
2. Il ne cherche plus de mise à jour, donc il ne contacte plus aucun site.
3. Il accepte vos variables d'environnement (Paramètres > Système). Écrivez `MANGOHUD=1` sur une
   ligne et le jeu démarre avec.

Sous le capot, la page WebView2 de Windows a laissé place à du Rust avec egui.

## À qui revient le mérite

Pas à moi pour l'essentiel. Les pages, les réglages, les textes, l'installation et tout le portage PC
sont le travail de
**[AllenWalkerED/nier-reincarnation-pc](https://github.com/AllenWalkerED/nier-reincarnation-pc)**.
Je n'ai fait que l'adapter à Linux.

Et ce portage doit lui-même beaucoup à d'autres :

| Projet | Son rôle |
|---|---|
| [Lunar Tear](https://github.com/Walter-Sparrow/lunar-tear) | le serveur qui a fait revivre le jeu |
| [win.apk](https://github.com/israel2176/win.apk) | le runtime Android |
| [Bunker](https://github.com/veritr1x/bunker) | les patchs de l'APK et des changements du serveur |
| [Lunar Base](https://github.com/NMeliksah/lunar-base), [Lunar Tear Comfort Mod](https://github.com/NavHobbyDev/lunar-tear-comfort-mod), [lunar-tear-masterdata-patcher](https://github.com/NavHobbyDev/lunar-tear-masterdata-patcher) | les mods, les boutiques et les événements |
| ANGLE, Eclipse Temurin, LLVM, texture2ddecoder, UnityPy, 7-Zip | les briques techniques |

Les licences de chacun sont détaillées dans le `NOTICE.md` et le dossier `licenses/` du projet d'origine.

## Le compiler soi-même

Il faut Linux, Rust stable et `cargo`. Rien d'autre.

```sh
cargo build --release
cargo test
```

Le binaire sort dans `target/release/nier-launcher`. Posez-le à la racine du dossier du jeu, celui que
`scripts/build_linux.sh` du projet d'origine assemble. Envie d'essayer avant d'installer ?

```sh
NIER_ROOT=<dossier du jeu> target/release/nier-launcher
```

## Côté légal

Le code est sous AGPL-3.0-only, la licence du projet d'origine.

C'est un projet de fans, non officiel et non commercial, sans aucun lien avec SQUARE ENIX. Le jeu
n'est pas fourni : il vous faut votre propre copie de NieR Re[in]carnation.
