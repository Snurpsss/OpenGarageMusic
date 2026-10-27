# Traduire

*[English version](../en/translating.md)*

Les textes du code sont en anglais, et c'est l'anglais qui s'affiche tant qu'aucune
autre langue n'est choisie. Chaque autre langue est un catalogue
[gettext](https://www.gnu.org/software/gettext/manual/html_node/PO-Files.html)
dans `apps/opengarage/lang` : `fr.po`, `de.po`, `es.po`, `it.po`, `pt.po`.
La langue se choisit dans **View → Language** et elle est mémorisée.

## Ajouter une langue

1. Copier `apps/opengarage/lang/template.pot` en `<code>.po`, où `<code>` est le code
   de la langue (`nl`, `pl`, `ja`…).
2. Dans l'en-tête, mettre le code dans `Language:` et, dans `X-Language-Name:`, le nom
   de la langue dans cette langue (`Nederlands`). C'est ce nom que le menu affiche.
3. Remplir chaque `msgstr`. N'importe quel éditeur gettext (Poedit, Lokalize, Weblate…)
   convient, un éditeur de texte aussi.
4. Soit recompiler le programme — tout fichier `.po` du dossier est intégré, rien
   d'autre n'est à modifier —, soit, sans recompiler, déposer le fichier dans un
   dossier `lang` à côté du programme, ou dans `%APPDATA%\OpenGarageMusic\lang`.

Un fichier sur le disque portant le code d'une langue intégrée la remplace : c'est le
moyen de corriger une traduction sans recompiler.

## Règles

- Un texte sans traduction (`msgstr` vide, ou entrée absente) s'affiche en anglais.
- Garder les emplacements de valeurs tels quels : `{}`, `{count}`, `{:.1}`. Ils peuvent
  changer de place dans la phrase. Quand deux `{}` doivent s'échanger, les numéroter à
  partir de zéro : `{1} … {0}`.
- Les entrées de menu portent leur raccourci clavier après une série d'espaces
  (`Undo        Ctrl+Z`) : garder les espaces et le raccourci.
- Les espaces au début ou à la fin d'un texte en font partie.
- Ce que les greffons et les banques de sons disent d'eux-mêmes (leur nom, le nom de
  leurs réglages) s'affiche tel qu'ils le fournissent, le plus souvent en anglais ; ce
  n'est pas traduit.
- Quelques messages d'erreur de bas niveau du moteur audio (fichier WAV, MIDI ou
  SoundFont illisible) n'existent qu'en anglais.

## Pour les développeurs

- Un texte s'écrit `tr("Text")`. Avec des valeurs : `tf!("Track {}", numero)` ; les
  valeurs que le texte nomme sont listées entre crochets :
  `tf!("{count} notes recorded." [count])`.
- Les noms qui viennent du moteur (sons, boucles, modèles, types de piste) sont des
  identifiants en anglais ; ils passent par `tr(nom)` là où ils sont affichés, jamais
  là où ils sont enregistrés ou comparés.
- Un nouveau texte doit être ajouté à `template.pot` et à chaque fichier `.po` du
  dossier. `dev test -p opengaragemusic` échoue, en nommant les textes manquants, tant
  que chaque langue intégrée n'a pas tous les textes du code avec leurs emplacements
  de valeurs intacts.

## État des traductions

Le français reprend les textes que le programme avait avant d'être traduit. L'allemand,
l'espagnol, l'italien et le portugais ont été écrits avec l'aide d'une IA et n'ont pas
été relus par des locuteurs natifs : les corrections sont bienvenues.
