# Thèmes

[English](../en/themes.md)

Un thème règle les couleurs de l'interface. Trois sont fournis avec le programme,
dans `apps/opengarage/themes` :

| Fichier | Nom | Ce que c'est |
|---|---|---|
| `studio.json` | Studio | L'aspect propre au programme, et le thème par défaut |
| `dark.json` | Sombre | Plus sombre et plus contrasté |
| `light.json` | Clair | Texte sombre sur fond clair |

Le thème se choisit dans **Présentation → Thème**. Le choix est conservé d'une
session à l'autre.

## Créer le sien

1. **Présentation → Thème → Enregistrer ce thème comme modèle…** écrit le thème en
   cours dans un fichier qui donne toutes les couleurs.
2. Modifiez les couleurs dans un éditeur de texte.
3. **Présentation → Thème → Importer un thème (fichier .json)…** copie le fichier
   dans le dossier des thèmes du programme (`%APPDATA%\OpenGarageMusic\themes`) et
   bascule dessus.

Le nom du fichier, sans `.json`, identifie le thème : importer un fichier de même
nom le remplace, et un fichier nommé comme un thème fourni (`dark.json`) prend sa
place. **Présentation → Thème → Ouvrir le dossier des thèmes** montre le dossier ;
un fichier qu'on y supprime n'est plus proposé.

## Le fichier

```json
{
  "name": "Sable du soir",
  "base": "light",
  "colors": {
    "panel": "#e8dcc0",
    "record": "#aa2200"
  },
  "tracks": ["#336699", "#4fae69"]
}
```

- `name` : ce que le menu affiche. Sans lui, le nom du fichier.
- `base` : `dark` (sombre) ou `light` (clair). Tout ce que le fichier ne donne pas
  en est repris : un thème peut donc tenir en quelques lignes.
- `colors` : n'importe lesquelles des couleurs ci-dessous, sous la forme `#rrggbb`
  (ou `#rgb`, ou `#rrggbbaa` avec transparence). Une couleur que cette version ne
  connaît pas est ignorée ; une valeur qui n'est pas une couleur fait échouer
  l'import, et le message la nomme.
- `tracks` : jusqu'à huit couleurs, pour les pistes et leurs régions. Les premières
  servent aussi d'accents : la deuxième dessine les potentiomètres et les notes
  sélectionnées, la première les indicateurs de niveau.

| Couleur | Ce qu'elle peint |
|---|---|
| `window` | Fond des fenêtres et des menus |
| `panel` | Fond des panneaux |
| `field` | Champs de texte, couloirs des pistes, éditeurs |
| `stripe` | En-têtes de piste et zones légèrement en relief |
| `button`, `button_hover`, `button_active` | Boutons : au repos, sous le pointeur, enfoncés |
| `button_text` | Texte des boutons |
| `border` | Lignes entre les panneaux, bords des fenêtres |
| `text` | Texte ordinaire |
| `text_strong` | Titres et texte mis en avant |
| `selection`, `selection_text` | Ce qui est sélectionné ou activé |
| `link` | Liens |
| `outline` | Contour de la région sélectionnée, courbes d'automatisation |
| `record` | Le bouton d'enregistrement, les erreurs |
| `warning` | Avertissements |
| `done` | Ce qui a réussi ou est déjà là |

Un thème ne règle que les couleurs : ni les tailles, ni les polices, ni les formes.
Les régions portent toujours un texte sombre : les couleurs de `tracks` gagnent donc
à rester plutôt claires.
