/* ЛБ3, задача 2: поиск в одномерном массиве.
 * Сколько раз встречается заданное число и его первый индекс. */
#include <stdio.h>

#define N_MAX 100

int main(void) {
    int a[N_MAX];
    int n = 0;
    int i = 0;
    int key = 0;
    int count = 0;
    int first = -1;

    while (1) {
        printf("размер (1..%d): ", N_MAX);
        if (scanf("%d", &n) == 1 && n >= 1 && n <= N_MAX) {
            break;
        }
        printf("плохой размер, повтори\n");
        while (getchar() != '\n') {
        }
    }
    printf("элементы массива:\n");
    for (i = 0; i < n; i++) {
        printf("a[%d] = ", i);
        scanf("%d", &a[i]);
    }
    printf("что ищем: ");
    scanf("%d", &key);
    for (i = 0; i < n; i++) {
        if (a[i] == key) {
            count = count + 1;
            if (first == -1) {
                first = i;
            }
        }
    }
    if (count == 0) {
        printf("число %d не найдено\n", key);
    } else {
        printf("найдено %d раз, первый индекс %d\n", count, first);
    }
    return 0;
}
